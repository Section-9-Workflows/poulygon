// SPDX-License-Identifier: MIT
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/input.h>
#include <stdint.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <sys/sysmacros.h>
#include <unistd.h>

struct facet_input_event { uint16_t kind, code; int32_t value; };

static int has_bit(const unsigned char *bits, unsigned bit)
{
    return !!(bits[bit / 8] & (1u << (bit % 8)));
}

int facet_input_open(const char *path)
{
    int fd = open(path, O_RDONLY | O_NONBLOCK | O_CLOEXEC | O_NOFOLLOW);
    if (fd < 0) return -errno;
    struct stat st;
    unsigned char keys[(KEY_MAX + 8) / 8] = {0};
    if (fstat(fd, &st) || !S_ISCHR(st.st_mode) || major(st.st_rdev) != 13 ||
        ioctl(fd, EVIOCGBIT(EV_KEY, sizeof(keys)), keys) < 0 ||
        !(has_bit(keys, KEY_A) || has_bit(keys, BTN_LEFT))) {
        close(fd);
        return -ENODEV;
    }
    return fd;
}

int facet_input_read(int fd, struct facet_input_event *output)
{
    struct input_event event;
    ssize_t size = read(fd, &event, sizeof(event));
    if (size < 0) return errno == EAGAIN || errno == EINTR ? 0 : -errno;
    if (size != sizeof(event)) return -EIO;
    *output = (struct facet_input_event){event.type, event.code, event.value};
    return 1;
}

int facet_input_keys(int fd, unsigned char *keys, uint32_t size)
{
    if (size != (KEY_MAX + 8) / 8) return -EINVAL;
    if (ioctl(fd, EVIOCGKEY(size), keys) < 0) return -errno;
    return 0;
}

void facet_input_close(int fd) { close(fd); }
