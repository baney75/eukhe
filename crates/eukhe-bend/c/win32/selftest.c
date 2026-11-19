// The lazy mappings of c/win32/sys/mman.h checked in place (Windows; the
// crate's ffi tests call it): a lazy mapping's pages commit with the
// protection it asked for, a PROT_NONE one commits nothing, mprotect of a
// lazy mapping leaves its pages reserved, and a guard is a fault. It has
// its own mappings and fault handler (its own copy of the header's
// state), so it touches none of the runtime's.
#include <stdint.h>
#include <sys/mman.h>

#define EB_W32_TEST_SIZE ((size_t)4 << 20)

static uint32_t eb_w32_test_state(const void* at, uint32_t* protect) {
  eb_w32_memory_info info = { 0 };
  VirtualQuery(at, &info, sizeof info);
  *protect = info.protect;
  return info.state;
}

// 0 when every check holds, else the number of the first that fails.
int eukhe_bend_win32_selftest(void) {
  uint32_t page = 0;
  // 1: a read-only lazy mapping commits read-only on a read.
  volatile char* ro = mmap(NULL, EB_W32_TEST_SIZE, PROT_READ,
    MAP_PRIVATE | MAP_ANON | MAP_NORESERVE, -1, 0);
  if (ro == MAP_FAILED || ro[5000] != 0
    || eb_w32_test_state((const void*)(ro + 5000), &page) != EB_W32_MEM_COMMIT
    || page != EB_W32_PAGE_READONLY) {
    return 1;
  }
  // 2: a write to it is a fault, never a commit read-write.
  if (eb_w32_fault_at((uintptr_t)(ro + 5000), 1) != EB_W32_TRAP) {
    return 2;
  }
  // 3: a PROT_NONE lazy mapping commits nothing on a touch: a fault.
  char* none = mmap(NULL, EB_W32_TEST_SIZE, PROT_NONE,
    MAP_PRIVATE | MAP_ANON | MAP_NORESERVE, -1, 0);
  if (none == MAP_FAILED || eb_w32_fault_at((uintptr_t)(none + 100), 0) != EB_W32_TRAP
    || eb_w32_test_state(none + 100, &page) != EB_W32_MEM_RESERVE) {
    return 3;
  }
  // 4: mprotect of it to read-write leaves it reserved; a write commits.
  if (mprotect(none, EB_W32_TEST_SIZE, PROT_READ | PROT_WRITE) != 0
    || eb_w32_test_state(none + (3 << 20), &page) != EB_W32_MEM_RESERVE) {
    return 4;
  }
  none[10] = 1;
  if (eb_w32_test_state(none + 10, &page) != EB_W32_MEM_COMMIT
    || page != EB_W32_PAGE_READWRITE) {
    return 5;
  }
  // 6: a PROT_NONE guard in a read-write lazy mapping is a fault.
  char* rw = mmap(NULL, EB_W32_TEST_SIZE, PROT_READ | PROT_WRITE,
    MAP_PRIVATE | MAP_ANON | MAP_NORESERVE, -1, 0);
  if (rw == MAP_FAILED || mprotect(rw + (2 << 20), 16384, PROT_NONE) != 0
    || eb_w32_fault_at((uintptr_t)(rw + (2 << 20) + 8), 1) != EB_W32_TRAP) {
    return 6;
  }
  rw[10] = 1;
  if (eb_w32_test_state(rw + 10, &page) != EB_W32_MEM_COMMIT
    || page != EB_W32_PAGE_READWRITE) {
    return 7;
  }
  munmap((void*)ro, EB_W32_TEST_SIZE);
  munmap(none, EB_W32_TEST_SIZE);
  munmap(rw, EB_W32_TEST_SIZE);
  return 0;
}
