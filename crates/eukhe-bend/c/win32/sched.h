// sched_yield for core.c on Windows. sched_getaffinity and cpu_set_t are
// read under __linux__ only.
#ifndef EUKHE_BEND_SCHED_H
#define EUKHE_BEND_SCHED_H

#include "eb_win32.h"

static inline int sched_yield(void) {
  SwitchToThread();
  return 0;
}

#endif
