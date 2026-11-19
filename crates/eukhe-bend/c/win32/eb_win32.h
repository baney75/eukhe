// The Win32 calls the POSIX compat headers of this directory make
// (crates/eukhe-bend build.rs puts it first on the include path for a
// Windows target, which clang builds).
//
// The headers declare these kernel32 functions themselves instead of
// including <windows.h>: windows.h defines FAR, near, far, small and many
// more as macros, and core.c defines and uses names it would rewrite. The
// prototypes and struct layouts follow the Windows SDK for x86-64 and
// AArch64 (LLP64); the types keep an eb_w32_ prefix so no SDK name is taken.
#ifndef EUKHE_BEND_WIN32_H
#define EUKHE_BEND_WIN32_H

#include <stddef.h>
#include <stdint.h>

#define EB_W32_WINAPI __declspec(dllimport) __stdcall

typedef struct {
  void* ptr;
} eb_w32_srwlock;  // SRWLOCK and CONDITION_VARIABLE: one pointer, zero-initialized

typedef struct {
  void*    base;             // BaseAddress
  void*    allocation_base;  // AllocationBase
  uint32_t allocation_protect;
  uint16_t partition_id;
  size_t   region_size;
  uint32_t state;
  uint32_t protect;
  uint32_t type;
} eb_w32_memory_info;  // MEMORY_BASIC_INFORMATION

typedef struct eb_w32_exception_record {
  uint32_t                    code;
  uint32_t                    flags;
  struct eb_w32_exception_record* next;
  void*                       address;
  uint32_t                    parameters;
  uintptr_t                   information[15];
} eb_w32_exception_record;  // EXCEPTION_RECORD

typedef struct {
  eb_w32_exception_record* record;
  void*                context;
} eb_w32_exception_pointers;  // EXCEPTION_POINTERS

typedef long(__stdcall* eb_w32_exception_handler)(eb_w32_exception_pointers*);
typedef uint32_t(__stdcall* eb_w32_thread_start)(void*);

#define EB_W32_INFINITE                     0xFFFFFFFFu
#define EB_W32_MEM_COMMIT                   0x00001000u
#define EB_W32_MEM_RESERVE                  0x00002000u
#define EB_W32_MEM_RELEASE                  0x00008000u
#define EB_W32_PAGE_NOACCESS                0x01u
#define EB_W32_PAGE_READONLY                0x02u
#define EB_W32_PAGE_READWRITE               0x04u
#define EB_W32_PAGE_EXECUTE_READWRITE       0x40u
#define EB_W32_EXCEPTION_ACCESS_VIOLATION   0xC0000005u
#define EB_W32_EXCEPTION_CONTINUE_EXECUTION (-1)
#define EB_W32_EXCEPTION_CONTINUE_SEARCH    0
#define EB_W32_ALL_PROCESSOR_GROUPS         0xFFFFu
#define EB_W32_STACK_IS_A_RESERVATION       0x00010000u

EB_W32_WINAPI void*    VirtualAlloc(void* at, size_t size, uint32_t type, uint32_t protect);
EB_W32_WINAPI int      VirtualFree(void* at, size_t size, uint32_t type);
EB_W32_WINAPI int      VirtualProtect(void* at, size_t size, uint32_t protect, uint32_t* old);
EB_W32_WINAPI size_t   VirtualQuery(const void* at, eb_w32_memory_info* info, size_t size);
EB_W32_WINAPI void*    AddVectoredExceptionHandler(uint32_t first, eb_w32_exception_handler handler);
EB_W32_WINAPI void     AcquireSRWLockExclusive(eb_w32_srwlock* lock);
EB_W32_WINAPI void     ReleaseSRWLockExclusive(eb_w32_srwlock* lock);
EB_W32_WINAPI int      SleepConditionVariableSRW(eb_w32_srwlock* cv, eb_w32_srwlock* lock, uint32_t ms,
  uint32_t flags);
EB_W32_WINAPI void     WakeConditionVariable(eb_w32_srwlock* cv);
EB_W32_WINAPI void     WakeAllConditionVariable(eb_w32_srwlock* cv);
EB_W32_WINAPI void*    CreateThread(void* security, size_t stack, eb_w32_thread_start start, void* arg,
  uint32_t flags, uint32_t* id);
EB_W32_WINAPI int      CloseHandle(void* handle);
EB_W32_WINAPI uint32_t GetCurrentThreadId(void);
EB_W32_WINAPI int      SwitchToThread(void);
EB_W32_WINAPI void     Sleep(uint32_t ms);
EB_W32_WINAPI int      QueryPerformanceCounter(int64_t* count);
EB_W32_WINAPI int      QueryPerformanceFrequency(int64_t* frequency);
EB_W32_WINAPI uint32_t GetActiveProcessorCount(uint16_t group);
EB_W32_WINAPI uint32_t GetModuleFileNameA(void* module, char* path, uint32_t size);

#endif
