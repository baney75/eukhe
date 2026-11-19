// POSIX threads for core.c on Windows: mutexes over SRW locks, condition
// variables over CONDITION_VARIABLE, threads over CreateThread. Only what
// core.c calls. A mutex is not recursive, as the default pthread mutex.
// Eukhe runs the core on one lane (crates/eukhe-bend/c/shim.c), so it
// starts no thread; the mutexes and condition variables guard its entries
// and its one-lane pool turns.
#ifndef EUKHE_BEND_PTHREAD_H
#define EUKHE_BEND_PTHREAD_H

#include <errno.h>
#include <stdlib.h>

#include "eb_win32.h"

// The thread id: what pthread_self answers and err_claim compares. A
// thread's handle closes at its start (no pthread_join here), so
// pthread_detach has nothing left to release.
typedef uintptr_t pthread_t;
typedef struct {
  int unused;
} pthread_attr_t;
typedef eb_w32_srwlock pthread_mutex_t;
typedef eb_w32_srwlock pthread_cond_t;

#define PTHREAD_MUTEX_INITIALIZER { 0 }
#define PTHREAD_COND_INITIALIZER  { 0 }

// A thread's machine stack: the 8 MiB a Linux thread gets by default,
// reserved, not committed.
#define EB_W32_THREAD_STACK (8u << 20)

typedef struct {
  void* (*run)(void*);
  void* arg;
} eb_w32_thread_job;

static uint32_t __stdcall eb_w32_thread_main(void* job) {
  eb_w32_thread_job run = *(eb_w32_thread_job*)job;
  free(job);
  run.run(run.arg);
  return 0;
}

static inline int pthread_create(pthread_t* tid, const pthread_attr_t* attr,
  void* (*run)(void*), void* arg) {
  (void)attr;
  eb_w32_thread_job* job = malloc(sizeof *job);
  if (job == NULL) {
    return EAGAIN;
  }
  *job       = (eb_w32_thread_job){ run, arg };
  uint32_t id = 0;
  void* thread = CreateThread(NULL, EB_W32_THREAD_STACK, eb_w32_thread_main, job,
    EB_W32_STACK_IS_A_RESERVATION, &id);
  if (thread == NULL) {
    free(job);
    return EAGAIN;
  }
  CloseHandle(thread);
  *tid = id;
  return 0;
}

static inline int pthread_detach(pthread_t tid) {
  (void)tid;
  return 0;
}

static inline pthread_t pthread_self(void) {
  return GetCurrentThreadId();
}

static inline int pthread_mutex_lock(pthread_mutex_t* m) {
  AcquireSRWLockExclusive(m);
  return 0;
}

static inline int pthread_mutex_unlock(pthread_mutex_t* m) {
  ReleaseSRWLockExclusive(m);
  return 0;
}

static inline int pthread_cond_wait(pthread_cond_t* cv, pthread_mutex_t* m) {
  // A wake may be spurious, as POSIX allows: every caller waits in a loop.
  SleepConditionVariableSRW(cv, m, EB_W32_INFINITE, 0);
  return 0;
}

static inline int pthread_cond_signal(pthread_cond_t* cv) {
  WakeConditionVariable(cv);
  return 0;
}

static inline int pthread_cond_broadcast(pthread_cond_t* cv) {
  WakeAllConditionVariable(cv);
  return 0;
}

#endif
