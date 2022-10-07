TARGET := riscv64gc-unknown-none-elf
TARGET_DIR := target/$(TARGET)/release
APP_DIR := src/bin
APPS := $(wildcard $(APP_DIR)/*.rs)
ELFS := $(patsubst $(APP_DIR)/%.rs, $(TARGET_DIR)/%, $(APPS))
BINS := $(patsubst $(APP_DIR)/%.rs, $(TARGET_DIR)/%.bin, $(APPS))

OBJCOPY := rust-objcopy --binary-architecture=riscv64

build: bin

bin: elf
	@$(foreach elf, $(ELFS), $(OBJCOPY) $(elf) --strip-all -O binary $(patsubst $(TARGET_DIR)/%, $(TARGET_DIR)/%.bin, $(elf));)

elf: 
	@cargo build --offline --release

clean:
	@cargo clean


