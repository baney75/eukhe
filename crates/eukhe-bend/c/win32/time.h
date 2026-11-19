// The UCRT's <time.h> plus clock_gettime(CLOCK_MONOTONIC) over
// QueryPerformanceCounter and nanosleep over Sleep (rounded up to whole
// milliseconds; it never returns early, so no time remains).
#ifndef EUKHE_BEND_TIME_H
#define EUKHE_BEND_TIME_H

#include_next <time.h>

#include <errno.h>

#include "eb_win32.h"

#define CLOCK_MONOTONIC 1

typedef int clockid_t;

static inline int clock_gettime(clockid_t clock, struct timespec* ts) {
  int64_t count = 0;
  int64_t hz    = 0;
  if (clock != CLOCK_MONOTONIC || !QueryPerformanceFrequency(&hz)
    || !QueryPerformanceCounter(&count)) {
    errno = EINVAL;
    return -1;
  }
  ts->tv_sec  = count / hz;
  ts->tv_nsec = (long)(count % hz * 1000000000 / hz);
  return 0;
}

static inline int nanosleep(const struct timespec* req, struct timespec* rem) {
  if (req->tv_sec < 0 || req->tv_nsec < 0 || req->tv_nsec > 999999999) {
    errno = EINVAL;
    return -1;
  }
  uint64_t ms = (uint64_t)req->tv_sec * 1000 + ((uint64_t)req->tv_nsec + 999999) / 1000000;
  Sleep(ms < EB_W32_INFINITE ? (uint32_t)ms : EB_W32_INFINITE - 1);
  if (rem != NULL) {
    rem->tv_sec  = 0;
    rem->tv_nsec = 0;
  }
  return 0;
}

#endif
