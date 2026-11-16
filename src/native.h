// SPDX-License-Identifier: MIT
// Copyright 2026 DoubleDensity <doubledense@gmail.com>
#ifndef FACET_NATIVE_H
#define FACET_NATIVE_H
#include <stdint.h>
struct facet;
struct facet_vertex { float position[4], color[4]; };
struct facet *facet_open(uint32_t *width, uint32_t *height);
int facet_frame(struct facet *, const struct facet_vertex *, uint32_t count);
int facet_close(struct facet *);
double facet_cpu_seconds(void);
#endif
