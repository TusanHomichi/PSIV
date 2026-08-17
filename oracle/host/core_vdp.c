#define _GNU_SOURCE

#include "core_vdp.h"

#include <dlfcn.h>
#include <elf.h>
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char g_error[256];

static void failf(const char *message)
{
	snprintf(g_error, sizeof g_error, "%s", message);
}

static int read_at(FILE *file, void *buffer, size_t size, long offset)
{
	if (fseek(file, offset, SEEK_SET) != 0 ||
	    fread(buffer, 1, size, file) != size) {
		failf("cannot read the core ELF symbol table");
		return -1;
	}
	return 0;
}

static int local_symbol_address(const char *path, uintptr_t base,
	                                const char *wanted, uintptr_t *address)
{
	FILE *file = NULL;
	Elf64_Ehdr header;
	Elf64_Shdr *sections = NULL;
	char *strings = NULL;
	int result = -1;
	uint16_t section;

	file = fopen(path, "rb");
	if (!file) {
		failf("cannot open the loaded core ELF for VDP symbols");
		return -1;
	}
	if (read_at(file, &header, sizeof header, 0) != 0 ||
	    memcmp(header.e_ident, ELFMAG, SELFMAG) != 0 ||
	    header.e_ident[EI_CLASS] != ELFCLASS64 ||
	    header.e_shentsize != sizeof(Elf64_Shdr) || header.e_shnum == 0) {
		failf("the loaded core is not a supported ELF64 image");
		goto done;
	}
	if (header.e_shoff > LONG_MAX) {
		failf("the core ELF section table is out of range");
		goto done;
	}
	sections = malloc((size_t)header.e_shnum * sizeof(*sections));
	if (!sections) {
		failf("out of memory reading the core ELF section table");
		goto done;
	}
	if (read_at(file, sections,
	            (size_t)header.e_shnum * sizeof(*sections),
	            (long)header.e_shoff) != 0)
		goto done;

	for (section = 0; section < header.e_shnum; section++) {
		Elf64_Shdr symbol_section = sections[section];
		Elf64_Shdr string_section;
		Elf64_Sym *symbols = NULL;
		size_t count;
		size_t i;

		if (symbol_section.sh_type != SHT_SYMTAB &&
		    symbol_section.sh_type != SHT_DYNSYM)
			continue;
		if (symbol_section.sh_link >= header.e_shnum ||
		    symbol_section.sh_size % sizeof(Elf64_Sym) != 0 ||
		    symbol_section.sh_size > SIZE_MAX) {
			continue;
		}
		string_section = sections[symbol_section.sh_link];
		if (string_section.sh_size == 0 || string_section.sh_size > SIZE_MAX ||
		    string_section.sh_size > (size_t)LONG_MAX ||
		    symbol_section.sh_offset > (uint64_t)LONG_MAX ||
		    string_section.sh_offset > (uint64_t)LONG_MAX)
			continue;
		strings = malloc((size_t)string_section.sh_size);
		if (!strings) {
			failf("out of memory reading the core ELF strings");
			goto done;
		}
		if (read_at(file, strings, (size_t)string_section.sh_size,
		            (long)string_section.sh_offset) != 0)
			goto done;
		count = (size_t)(symbol_section.sh_size / sizeof(Elf64_Sym));
		if (symbol_section.sh_size > (size_t)LONG_MAX) {
			free(strings);
			strings = NULL;
			continue;
		}
		symbols = malloc((size_t)symbol_section.sh_size);
		if (!symbols) {
			failf("out of memory reading the core ELF symbols");
			goto done;
		}
		if (read_at(file, symbols, (size_t)symbol_section.sh_size,
		            (long)symbol_section.sh_offset) != 0) {
			free(symbols);
			goto done;
		}
		for (i = 0; i < count; i++) {
			Elf64_Sym symbol = symbols[i];
			if (symbol.st_name >= string_section.sh_size ||
			    symbol.st_shndx == SHN_UNDEF ||
			    strcmp(strings + symbol.st_name, wanted) != 0)
				continue;
			*address = base + (uintptr_t)symbol.st_value;
			result = 0;
			break;
		}
		free(symbols);
		free(strings);
		strings = NULL;
		if (result == 0)
			break;
	}

	if (result != 0)
		failf("the core ELF has no requested VDP symbol");
done:
	free(strings);
	free(sections);
	if (file)
		fclose(file);
	return result;
}

int core_vdp_bind(void *core_anchor, struct core_vdp *vdp)
{
	Dl_info info;
	uintptr_t address;

	if (!vdp || !core_anchor || !dladdr(core_anchor, &info) ||
	    !info.dli_fname || !info.dli_fbase) {
		failf("cannot locate the loaded core for VDP symbols");
		return -1;
	}
	memset(vdp, 0, sizeof *vdp);
	if (local_symbol_address(info.dli_fname, (uintptr_t)info.dli_fbase,
	                        "reg", &address) != 0)
		return -1;
	vdp->registers = (const uint8_t *)address;
	if (local_symbol_address(info.dli_fname, (uintptr_t)info.dli_fbase,
	                        "vram", &address) != 0)
		return -1;
	vdp->vram = (const uint8_t *)address;
	if (local_symbol_address(info.dli_fname, (uintptr_t)info.dli_fbase,
	                        "vsram", &address) != 0)
		return -1;
	vdp->vsram = (const uint8_t *)address;
	if (local_symbol_address(info.dli_fname, (uintptr_t)info.dli_fbase,
                        "hscb", &address) != 0)
		return -1;
	vdp->hscroll_base = (const uint16_t *)address;
	if (local_symbol_address(info.dli_fname, (uintptr_t)info.dli_fbase,
	                        "hscroll_mask", &address) != 0)
		return -1;
	vdp->hscroll_mask = (const uint8_t *)address;
	return 0;
}

const char *core_vdp_error(void)
{
	return g_error[0] ? g_error : "unknown core VDP error";
}
