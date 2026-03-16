#!/bin/bash
set -euo pipefail

cargo espflash save-image --release --chip=esp32s3 --target=xtensa-esp32s3-none-elf --package trusty-s3 --bin trusty-s3 firmware.bin
if [[ $(stat -c%s firmware.bin) -gt 6553600 ]]; then
    echo -e "\033[0;31m[ERROR] Firmware size exceeds OFW partition limit!"
    exit 1
fi
cargo espflash write-bin 0x10000 firmware.bin --monitor
