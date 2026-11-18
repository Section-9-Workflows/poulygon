// SPDX-License-Identifier: MIT
// Copyright 2026 DoubleDensity <doubledense@gmail.com>
#ifndef FACET_NATIVE_H
#define FACET_NATIVE_H
#include <stdint.h>
struct facet;
struct facet_vertex { float position[4], color[4]; };
struct facet *facet_open(uint32_t *width, uint32_t *height);
struct facet_metrics { double construct_ms, submit_ms, completion_ms, present_ms; uint32_t draws; };
uint32_t facet_vertex_capacity(void);
uint32_t facet_indexed_capacity(void);
int facet_read_input(struct facet *, unsigned char *bytes, uint32_t capacity);
int facet_frame(struct facet *, const struct facet_vertex *, uint32_t count, struct facet_metrics *);
int facet_close(struct facet *);
double facet_cpu_seconds(void);
#endif
