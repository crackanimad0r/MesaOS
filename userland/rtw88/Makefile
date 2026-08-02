CC = gcc
CFLAGS = -m64 -mcmodel=kernel -nostdinc -I include -fno-pic -fno-stack-protector -fno-builtin \
         -fno-omit-frame-pointer -O2 -Wall -Werror -c

# Files with unavoidable pointer signedness or compatibility issues
CFLAGS_fw = -Wno-error=incompatible-pointer-types -Wno-error=maybe-uninitialized
CFLAGS_phy = -Wno-error=pointer-sign
CFLAGS_regd = -Wno-error=pointer-sign

# Core files (target: main, mac80211, util, tx, rx, mac, phy, coex, efuse, fw, ps, sec, bf, sar, regd)
CORE_OBJS = bf.o coex.o efuse.o fw.o mac.o mac80211.o main.o phy.o ps.o regd.o \
            rx.o sar.o sec.o tx.o util.o

# PCI files
PCI_OBJS = pci.o

# Table files (chip-specific data tables)
TABLE_OBJS = rtw8822c_table.o

# Chip files  
CHIP_OBJS = rtw8822c.o rtw8822ce.o

ALL_OBJS = $(CORE_OBJS) $(PCI_OBJS) $(TABLE_OBJS) $(CHIP_OBJS)

rtw88.ko: $(ALL_OBJS)
	ld -r -o rtw88.ko $(ALL_OBJS)
	@echo "[BUILD] rtw88.ko ready"
	@ls -la rtw88.ko

%.o: %.c
	$(CC) $(CFLAGS) $($(subst .o,,$@)_CFLAGS) $< -o $@

# Override for specific files with custom CFLAGS
fw.o: fw.c
	$(CC) $(CFLAGS) $(CFLAGS_fw) fw.c -o fw.o

phy.o: phy.c
	$(CC) $(CFLAGS) $(CFLAGS_phy) phy.c -o phy.o

regd.o: regd.c
	$(CC) $(CFLAGS) $(CFLAGS_regd) regd.c -o regd.o

clean:
	rm -f $(ALL_OBJS) rtw88.ko rtw8822c_table.o

.PHONY: clean
