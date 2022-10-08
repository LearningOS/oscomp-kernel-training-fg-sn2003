NAME 		:= kernel
MODE 		?= release
BOARD 		?= qemu
SBI 		?= rustsbi
CPUS		?= 2
TARGET 		:= riscv64gc-unknown-none-elf

#PATH
TARGET_DIR 	:= target/$(TARGET)/$(MODE)
KERNEL_ELF 	:= $(TARGET_DIR)/$(NAME)
KERNEL_BIN 	:= $(KERNEL_ELF).bin
OS_BIN		:= ../os.bin
ifeq ($(BOARD),qemu)
	BOOTLOADER := ../bootloader/opensbi-qemu.elf
else ifeq ($(BOARD),k210)
	BOOTLOADER := ../bootloader/rustsbi-k210.bin
endif

FS_IMG		:= ../fat32.img
BOOTLOADER_SIZE := 131072

# Run K210
K210-SERIALPORT	= /dev/ttyUSB0
K210-BURNER	= ../tools/kflash.py
SDCARD := /dev/sdc

#cargo feature
BOARD_FEATURE := board_$(BOARD)
FEATURES	  += $(BOARD_FEATURE) 

#kernel_entry
ifeq ($(BOARD),qemu)
	KERNEL_ENTRY := 0x80200000
else ifeq ($(BOARD),k210)
	KERNEL_ENTRY := 0x80020000
else ifeq ($(BOARD),sifive)
	KERNEL_ENTRY := 0x80200000
endif

#build tool
OBJCOPY 	:= rust-objcopy --binary-architecture=riscv64

#qemu
QEMU_MACHINE   	?= virt
QEMU 		:= qemu-system-riscv64
QEMU-ARGS	+= -machine virt
QEMU-ARGS	+= -kernel ../kernel-qemu
QEMU-ARGS	+= -smp 1
QEMU-ARGS	+= -m 256M
QEMU-ARGS	+= -nographic
QEMU-ARGS	+= -bios $(BOOTLOADER)
QEMU-ARGS	+= -device loader,file=$(KERNEL_BIN),addr=$(KERNEL_ENTRY)
QEMU-ARGS	+= -drive file=$(FS_IMG),if=none,format=raw,id=x0
QEMU-ARGS	+= -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0

sdcard: 
	@echo "Are you sure write to $(SDCARD) ? [y/N] " && read ans && [ $${ans:-N} = y ]
	@sudo dd if=/dev/zero of=$(SDCARD) bs=1048576 count=32
	@sudo dd if=$(FS_IMG) of=$(SDCARD)

run: build
ifeq ($(BOARD),k210)
	which $(K210-BURNER) || (cd .. && git clone https://github.com/sipeed/kflash.py.git && mv kflash.py tools)
	@sudo chmod 777 $(K210-SERIALPORT)
	python3 $(K210-BURNER) -p $(K210-SERIALPORT) -b 1500000 $(OS_BIN)
	python3 -m serial.tools.miniterm --eol LF --dtr 0 --rts 0 --filter direct $(K210-SERIALPORT) 115200
else ifeq ($(BOARD),qemu)
	@$(QEMU) $(QEMU-ARGS)
endif

build: env user_bin $(KERNEL_BIN)
ifeq ($(BOARD),k210)
	cp $(BOOTLOADER) $(BOOTLOADER).copy
	dd if=$(KERNEL_BIN) of=$(BOOTLOADER).copy bs=$(BOOTLOADER_SIZE) seek=1
	mv $(BOOTLOADER).copy $(OS_BIN)
else ifeq ($(BOARD),qemu)
	cp $(BOOTLOADER) ../sbi-qemu
	mv $(KERNEL_ELF) ../kernel-qemu
endif

debug: build
	@tmux new-session -d \
		"$(QEMU) $(QEMU-ARGS) -s -S" && \
		tmux split-window -h "riscv64-unknown-elf-gdb -ex 'file $(KERNEL_ELF)' -ex 'set arch riscv:rv64' -ex 'target remote localhost:1234'" && \
		tmux -2 attach-session -d

debug_remote: build
	@$(QEMU) $(QEMU-ARGS) -s -S

user_bin:
	@cd ../user && make build
	
clean:
	@cargo clean

kernel_elf: 
	@cp ./linker/linker-$(BOARD).ld ./linker/linker.ld
ifeq ($(MODE),debug)
	@cargo build --offline --features "$(FEATURES)"
else
	@cargo build --offline --release --features "$(FEATURES)"
endif

$(KERNEL_BIN): kernel_elf
	$(OBJCOPY) $(KERNEL_ELF) --strip-all -O binary $@

env:
	rustup target add riscv64gc-unknown-none-elf

.PHONY: env build clean elf debug