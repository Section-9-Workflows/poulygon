// SPDX-License-Identifier: MIT
// Copyright 2026 DoubleDensity <doubledense@gmail.com>
#define _GNU_SOURCE
#include "native.h"
#include "poulsback_frame.h"
#include "poulsback_bo_uapi.h"
#include <drm_fourcc.h>
#include <errno.h>
#include <fcntl.h>
#include <linux/kd.h>
#include <linux/sync_file.h>
#include <linux/vt.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/sysmacros.h>
#include <termios.h>
#include <time.h>
#include <unistd.h>
#include <xf86drm.h>
#include <xf86drmMode.h>

struct facet {
    int fd, kd;
    uint32_t width, height, stride, connector, context, syncobj, fb[2], next;
    struct poulsback_frame_bo color[2], depth, pds, vertices;
    struct poulsback_frame_writer *writer;
    poulsback_frame_shape shape;
    struct sgx535_fragment_instruction fragment[8];
    drmModeCrtc *saved;
    struct termios termios;
    struct vt_mode vt;
    uint32_t (*words)[8];
    bool terminal_saved, vt_saved, master, shown, pending, lost;
};
static volatile sig_atomic_t stop, release_vt;

uint32_t facet_vertex_capacity(void)
{
    return (POULSBACK_FRAME_VERTEX_BYTES / sizeof(struct facet_vertex)) / 3u * 3u;
}

static double milliseconds(void)
{
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    return now.tv_sec * 1000.0 + now.tv_nsec / 1e6;
}

double facet_cpu_seconds(void)
{
    struct timespec now;
    if (clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &now)) return 0;
    return now.tv_sec + now.tv_nsec * 1e-9;
}

static void interrupted(int signal)
{
    stop = 1;
    if (signal == SIGUSR1) release_vt = 1;
}

static int error(const char *where, int code)
{
    fprintf(stderr, "facet: %s: %s (%d)\n", where, strerror(code), code);
    return -code;
}

static void free_bo(void *data, struct poulsback_frame_bo *bo)
{
    struct facet *f = data;
    if (f->lost) return;
    if (bo->map) munmap(bo->map, (size_t)bo->size);
    if (bo->handle) {
        struct drm_gem_close request = {.handle = bo->handle};
        if (drmIoctl(f->fd, DRM_IOCTL_GEM_CLOSE, &request))
            error("close buffer", errno);
    }
    memset(bo, 0, sizeof(*bo));
}

static int create_bo(struct facet *f, uint32_t type, uint32_t flags, uint64_t bytes,
                     bool mapped, struct poulsback_frame_bo *out)
{
    struct poulsback_bo_create request = {
        .abi = POULSBACK_BO_ABI, .bo_class = type, .flags = flags,
        .size = (bytes + 4095u) & ~UINT64_C(4095)};
    if (drmIoctl(f->fd, DRM_IOCTL_PB_BO_CREATE, &request)) {
        int code = errno;
        if (code == EIO) f->lost = true;
        return -code;
    }
    *out = (struct poulsback_frame_bo){request.handle, request.gpu_va, request.size, NULL};
    if (request.size < bytes || request.size > SIZE_MAX) {
        free_bo(f, out);
        return -EOVERFLOW;
    }
    if (mapped) {
        void *map = mmap(NULL, (size_t)request.size, PROT_READ | PROT_WRITE, MAP_SHARED,
                         f->fd, (off_t)request.mmap_offset);
        if (map == MAP_FAILED) {
            int ret = -errno;
            free_bo(f, out);
            return ret;
        }
        out->map = map;
    }
    return 0;
}

static int alloc_bo(void *data, uint32_t type, uint64_t bytes, struct poulsback_frame_bo *out)
{
    return create_bo(data, type, POULSBACK_BO_CPU_WC, bytes, true, out);
}

static int idle(void *data)
{
    const struct facet *f = data;
    return f->lost ? -EIO : 0; /* Each frame's fence is drained before the next begins. */
}

static void flipped(int fd, unsigned sequence, unsigned sec, unsigned usec, void *data)
{
    (void)fd; (void)sequence; (void)sec; (void)usec;
    ((struct facet *)data)->pending = false;
}

static int wait_fd(int fd)
{
    struct pollfd p = {.fd = fd, .events = POLLIN};
    int ret;
    do { ret = poll(&p, 1, 2000); } while (ret < 0 && errno == EINTR);
    if (ret < 0) return -errno;
    if (!ret) return -ETIMEDOUT;
    return (p.revents & POLLIN) ? 0 : -EIO;
}

static int wait_flip(struct facet *f)
{
    drmEventContext events = {.version = 2, .page_flip_handler = flipped};
    while (f->pending) {
        int ret = wait_fd(f->fd);
        if (ret) return ret;
        if (drmHandleEvent(f->fd, &events)) return -errno;
    }
    return 0;
}

static int render_complete(struct facet *f)
{
    int fence = -1;
    if (drmSyncobjExportSyncFile(f->fd, f->syncobj, &fence)) return -errno;
    int ret = wait_fd(fence);
    struct sync_file_info info = {0};
    if (!ret && ioctl(fence, SYNC_IOC_FILE_INFO, &info)) ret = -errno;
    if (!ret && info.status != 1) ret = info.status < 0 ? info.status : -EIO;
    close(fence);
    return ret;
}

static uint32_t bits(float value)
{
    uint32_t word;
    memcpy(&word, &value, sizeof(word));
    return word;
}

static void setup_shape(struct facet *f)
{
    poulsback_frame_shape *s = &f->shape;
    s->mesh.version = SGX535_DEPTH_WINDOW_MESH_GL_WINDOW_VERSION;
    s->mesh.format = SGX535_DEPTH_D24S8;
    s->mesh.scene.version = SGX535_WINDOW_MESH_GL_WINDOW_BACKGROUND_VERSION;
    s->mesh.scene.packed_background_argb = 0xff101522;
    struct sgx535_window_mesh_request *m = &s->mesh.scene.foreground;
    m->version = SGX535_WINDOW_MESH_CLIP_VERSION;
    m->profile = SGX535_WINDOW_MESH_PROFILE;
    m->core_id = SGX535_ENGINE121_CORE_ID;
    m->revision = SGX535_ENGINE121_REVISION;
    m->width = f->width; m->height = f->height;
    m->order = SGX535_WINDOW_MESH_DEINDEXED;
    m->interpolation = SGX535_VARYING_FLAT_REPLICATED;
    m->window.origin = 1;
    m->window.half_pixel_center = 1;
    m->window.bottom_edge_rule = 1;
    m->window.sample_mask = UINT32_MAX;
    m->window.viewport_scale_f32[0] = bits(f->width * 0.5f);
    m->window.viewport_scale_f32[1] = bits(f->height * -0.5f);
    m->window.viewport_scale_f32[2] = bits(0.5f);
    m->window.viewport_translate_f32[0] = bits(f->width * 0.5f);
    m->window.viewport_translate_f32[1] = bits(f->height * 0.5f);
    m->window.viewport_translate_f32[2] = bits(0.5f);
    for (unsigned i = 0; i < 4; ++i) {
        f->fragment[i] = (struct sgx535_fragment_instruction){
            .opcode = SGX535_FRAGMENT_INPUT, .component = i};
        f->fragment[4+i] = (struct sgx535_fragment_instruction){
            .opcode = SGX535_FRAGMENT_OUTPUT, .index = i, .source = {i, 0, 0}};
    }
    m->fragment = f->fragment; m->fragment_count = 8;
    s->samples.version = SGX535_SINGLE_SAMPLE_SOURCE_VERSION;
    s->samples.effective_framebuffer_samples = 1;
    s->samples.fragment_instructions = 8;
}

static int display_open(struct facet *f)
{
    struct stat tty;
    struct vt_stat active;
    if (fstat(0, &tty) || !S_ISCHR(tty.st_mode) || major(tty.st_rdev) != 4 ||
        !minor(tty.st_rdev) || minor(tty.st_rdev) > 63 ||
        ioctl(0, VT_GETSTATE, &active) || minor(tty.st_rdev) != active.v_active)
        return error("run from a free Linux VT using the facet launcher", ENOTTY);
    if (ioctl(0, KDGETMODE, &f->kd) || f->kd != KD_TEXT) return -EBUSY;
    f->fd = open("/dev/dri/card0", O_RDWR | O_CLOEXEC);
    if (f->fd < 0) return -errno;
    drmVersion *version = drmGetVersion(f->fd);
    bool matched = version && version->name && !strcmp(version->name, "poulsback");
    if (version) drmFreeVersion(version);
    if (!matched) return -ENODEV;
    if (drmSetMaster(f->fd)) return -errno;
    f->master = true;
    struct poulsback_display_info info = {.abi = POULSBACK_BO_ABI};
    if (drmIoctl(f->fd, DRM_IOCTL_PB_DISPLAY_INFO, &info)) return -errno;
    if (!info.width || !info.height || info.width >= 2048 || info.height >= 2048 ||
        info.format != DRM_FORMAT_XRGB8888 || info.stride % 128 ||
        info.stride < info.width * 4u || info.stride > 8192) return -EOPNOTSUPP;
    f->width = info.width; f->height = info.height; f->stride = info.stride;
    drmModeRes *resources = drmModeGetResources(f->fd);
    if (!resources) return -errno;
    unsigned connected = 0;
    for (int i = 0; i < resources->count_connectors; ++i) {
        drmModeConnector *c = drmModeGetConnector(f->fd, resources->connectors[i]);
        drmModeEncoder *e = c && c->encoder_id ? drmModeGetEncoder(f->fd, c->encoder_id) : NULL;
        drmModeCrtc *crtc = e && e->crtc_id ? drmModeGetCrtc(f->fd, e->crtc_id) : NULL;
        if (c && c->connection == DRM_MODE_CONNECTED && crtc && crtc->mode_valid) {
            connected++;
            if (!f->saved) { f->saved = crtc; f->connector = c->connector_id; crtc = NULL; }
        }
        if (crtc) drmModeFreeCrtc(crtc);
        if (e) drmModeFreeEncoder(e);
        if (c) drmModeFreeConnector(c);
    }
    drmModeFreeResources(resources);
    if (connected != 1 || !f->saved || !f->saved->buffer_id || f->saved->x || f->saved->y ||
        f->saved->mode.hdisplay != f->width || f->saved->mode.vdisplay != f->height)
        return -EOPNOTSUPP;
    return 0;
}

struct facet *facet_open(uint32_t *width, uint32_t *height)
{
    struct facet *f = calloc(1, sizeof(*f));
    if (!f) return NULL;
    f->fd = -1;
    f->words = calloc(facet_vertex_capacity(), sizeof(*f->words));
    if (!f->words) { free(f); return NULL; }
    int ret = display_open(f);
    if (ret) goto failed;
    struct sigaction action = {.sa_handler = interrupted};
    sigemptyset(&action.sa_mask);
    if (sigaction(SIGTERM, &action, NULL) || sigaction(SIGINT, &action, NULL) ||
        sigaction(SIGHUP, &action, NULL) || sigaction(SIGUSR1, &action, NULL) ||
        sigaction(SIGUSR2, &action, NULL)) {
        ret = -errno; goto failed;
    }
    if (ioctl(0, VT_GETMODE, &f->vt) || tcgetattr(0, &f->termios)) { ret = -errno; goto failed; }
    f->terminal_saved = f->vt_saved = true;
    struct vt_mode mode = {.mode = VT_PROCESS, .relsig = SIGUSR1, .acqsig = SIGUSR2};
    struct termios raw = f->termios;
    cfmakeraw(&raw);
    if (ioctl(0, VT_SETMODE, &mode) || tcsetattr(0, TCSANOW, &raw)) { ret = -errno; goto failed; }
    struct poulsback_context_create context = {.abi = POULSBACK_EXEC_ABI};
    if (drmIoctl(f->fd, DRM_IOCTL_PB_CONTEXT_CREATE, &context)) { ret = -errno; goto failed; }
    f->context = context.handle;
    if (drmSyncobjCreate(f->fd, 0, &f->syncobj)) { ret = -errno; goto failed; }
    for (unsigned i = 0; i < 2; ++i) {
        ret = create_bo(f, POULSBACK_BO_CLASS_2D, POULSBACK_BO_CPU_WC | POULSBACK_BO_SCANOUT,
                        (uint64_t)f->stride * f->height, false, &f->color[i]);
        if (ret) goto failed;
        uint32_t handles[4] = {f->color[i].handle}, pitches[4] = {f->stride}, offsets[4] = {0};
        if (drmModeAddFB2(f->fd, f->width, f->height, DRM_FORMAT_XRGB8888, handles, pitches,
                          offsets, &f->fb[i], 0)) { ret = -errno; goto failed; }
    }
    ret = create_bo(f, POULSBACK_BO_CLASS_DEPTH, POULSBACK_BO_CPU_WC,
                    poulsback_frame_depth_bytes(f->width, f->height), false, &f->depth);
    if (ret) goto failed;
    ret = alloc_bo(f, POULSBACK_BO_CLASS_PDS, POULSBACK_FRAME_PDS_BYTES, &f->pds);
    if (ret) goto failed;
    ret = alloc_bo(f, POULSBACK_BO_CLASS_GENERAL, POULSBACK_FRAME_VERTEX_BYTES, &f->vertices);
    if (ret) goto failed;
    struct poulsback_frame_bo_ops ops = {f, alloc_bo, free_bo, idle};
    ret = poulsback_frame_writer_create(&ops, &f->writer);
    if (ret) goto failed;
    setup_shape(f);
    *width = f->width; *height = f->height;
    fprintf(stderr, "facet: native %ux%u, two scanout buffers; VT switch exits\n",
            f->width, f->height);
    return f;
failed:
    if (ret == -EIO) f->lost = true;
    error("initialization", -ret);
    facet_close(f);
    return NULL;
}

int facet_read_input(struct facet *f, unsigned char *bytes, uint32_t capacity)
{
    if (!f || !bytes || !capacity) return -EINVAL;
    if (stop) return 0;
    struct pollfd keyboard = {.fd = 0, .events = POLLIN};
    int ready = poll(&keyboard, 1, 0);
    if (ready < 0) return errno == EINTR ? 0 : -errno;
    if (!ready) return 0;
    if (keyboard.revents & (POLLERR | POLLHUP | POLLNVAL)) return -EIO;
    if (!(keyboard.revents & POLLIN)) return 0;
    ssize_t n = read(0, bytes, capacity);
    if (n < 0) return errno == EINTR || errno == EAGAIN ? 0 : -errno;
    return n ? (int)n : -EIO;
}

int facet_frame(struct facet *f, const struct facet_vertex *vertices, uint32_t count,
                struct facet_metrics *metrics)
{
    if (stop) return 0;
    memset(metrics, 0, sizeof(*metrics));
    if (!count || count % 3) return error("vertex span", EINVAL);
    if (count > facet_vertex_capacity()) return 2; /* Refused before submission. */
    double started = milliseconds();
    _Static_assert(sizeof(struct facet_vertex) == 32, "Rust/C vertex layout");
    memcpy(f->words, vertices, count * sizeof(*vertices));
    poulsback_frame_begin(f->writer, &f->pds, &f->vertices);
    int ret;
    const uint32_t batch = POULSBACK_FRAME_MAX_VERTICES / 3u * 3u;
    for (uint32_t offset = 0; offset < count;) {
        uint32_t size = count - offset < batch ? count - offset : batch;
        if (metrics->draws == POULSBACK_FRAME_MAX_DRAWS) return 2;
        struct poulsback_frame_vertices in = {
            .count = size, .window = &f->words[offset][0], .clip_w = &f->words[offset][3],
            .rgba = &f->words[offset][4], .window_stride = 8, .clip_stride = 8, .rgba_stride = 8};
        ret = poulsback_frame_draw(f->writer, &f->shape, &in);
        if (ret == -E2BIG || ret == -ENOSPC) return 2;
        if (ret) return error("construct draw", -ret);
        metrics->draws++;
        offset += size;
    }
    struct poulsback_frame_buffer target = {
        f->color[f->next].handle, f->stride, f->color[f->next].gpu_va, f->color[f->next].size};
    struct poulsback_frame_buffer depth = {f->depth.handle, 0, f->depth.gpu_va, f->depth.size};
    struct poulsback_frame_job job;
    ret = poulsback_frame_end(f->writer, &target, &depth, &job);
    if (ret == -E2BIG || ret == -ENOSPC) return 2;
    if (ret) return error("construct frame", -ret);
    metrics->construct_ms = milliseconds() - started;
    started = milliseconds();
    if (drmSyncobjReset(f->fd, &f->syncobj, 1)) return error("reset completed fence", errno);
    struct poulsback_submit submit = {
        .abi = POULSBACK_EXEC_ABI, .context = f->context, .width = job.width, .height = job.height,
        .bo_count = job.bo_count, .ta_reg_count = job.ta_count, .raster_reg_count = job.raster_count,
        .bos = (uintptr_t)job.bos, .ta_regs = (uintptr_t)job.ta, .raster_regs = (uintptr_t)job.raster,
        .out_syncobj = f->syncobj};
    if (drmIoctl(f->fd, DRM_IOCTL_PB_SUBMIT, &submit)) {
        int saved_errno = errno;
        f->lost = saved_errno == EIO;
        return error("submit", saved_errno);
    }
    metrics->submit_ms = milliseconds() - started;
    started = milliseconds();
    ret = render_complete(f);
    if (ret) { f->lost = true; return error("render completion; no retry", -ret); }
    metrics->completion_ms = milliseconds() - started;
    started = milliseconds();
    if (!f->shown) {
        if (drmModeSetCrtc(f->fd, f->saved->crtc_id, f->fb[f->next], 0, 0, &f->connector,
                           1, &f->saved->mode)) {
            f->lost = true;
            return error("first frame", errno);
        }
        f->shown = true;
        if (ioctl(0, KDSETMODE, KD_GRAPHICS)) return error("graphics VT", errno);
    } else {
        if (drmModePageFlip(f->fd, f->saved->crtc_id, f->fb[f->next], DRM_MODE_PAGE_FLIP_EVENT, f)) {
            f->lost = true;
            return error("page flip", errno);
        }
        f->pending = true;
        ret = wait_flip(f);
        if (ret) { f->lost = true; return error("page flip completion; no retry", -ret); }
    }
    metrics->present_ms = milliseconds() - started;
    f->next ^= 1u;
    return 1;
}

int facet_close(struct facet *f)
{
    if (!f) return 0;
    int result = 0;
    if (f->shown && !f->pending) {
        if (drmModeSetCrtc(f->fd, f->saved->crtc_id, f->saved->buffer_id,
                           f->saved->x, f->saved->y, &f->connector, 1, &f->saved->mode)) {
            result = error("restore console scanout", errno);
            f->lost = true;
        } else f->shown = false;
    }
    if (f->lost) {
        fprintf(stderr, "facet: uncertain device outcome; retaining GPU resources, no reset or retry\n");
        result = -EIO;
    } else {
        poulsback_frame_writer_destroy(f->writer);
        for (unsigned i = 0; i < 2; ++i) {
            if (f->fb[i] && drmModeRmFB(f->fd, f->fb[i])) result = error("retire framebuffer", errno);
            free_bo(f, &f->color[i]);
        }
        free_bo(f, &f->pds); free_bo(f, &f->vertices); free_bo(f, &f->depth);
        if (f->syncobj) drmSyncobjDestroy(f->fd, f->syncobj);
        if (f->context) {
            struct poulsback_context_destroy context = {.abi = POULSBACK_EXEC_ABI, .handle = f->context};
            if (drmIoctl(f->fd, DRM_IOCTL_PB_CONTEXT_DESTROY, &context))
                result = error("destroy context", errno);
        }
    }
    if (f->master && drmDropMaster(f->fd)) result = error("release display ownership", errno);
    if (f->terminal_saved) {
        if (ioctl(0, KDSETMODE, f->kd)) result = error("restore VT mode", errno);
        if (tcsetattr(0, TCSANOW, &f->termios)) result = error("restore keyboard", errno);
    }
    if (f->vt_saved) {
        if (release_vt) ioctl(0, VT_RELDISP, 1);
        if (ioctl(0, VT_SETMODE, &f->vt)) result = error("restore VT switching", errno);
    }
    if (f->saved) drmModeFreeCrtc(f->saved);
    if (f->fd >= 0) close(f->fd);
    free(f->words);
    free(f);
    return result;
}
