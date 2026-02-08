#include "ff.h"
#include <stddef.h>

bool ff_exists(const char* path) {
    FILINFO fno;
    FRESULT res = f_stat(path, &fno);
    return (res == FR_OK);
}

int ff_mount() {
    static FATFS fs;
    return f_mount(&fs, "", 1);
}

static_assert(sizeof(char) == 1, "char size mismatch");
static_assert(sizeof(BYTE) == 1, "BYTE size mismatch");
static_assert(sizeof(WORD) == 2, "WORD size mismatch");
static_assert(sizeof(DWORD) == 4, "DWORD size mismatch");
static_assert(sizeof(QWORD) == 8, "QWORD size mismatch");
static_assert(sizeof(WCHAR) == 2, "WCHAR size mismatch");
static_assert(sizeof(UINT) == 4, "UINT size mismatch");

static_assert(sizeof(FFOBJID) == 48, "FFOBJID size mismatch with Rust");
static_assert(sizeof(FIL) == 592, "FIL size mismatch with Rust");
static_assert(sizeof(DIR) == 80, "DIR size mismatch with Rust");
static_assert(sizeof(FILINFO) == 288, "FILINFO size mismatch with Rust");

