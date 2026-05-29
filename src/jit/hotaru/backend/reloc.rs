// ─────────────────────────────────────────────────────────────────────────────
// HotaruReloc — Forward-jump relocation entries
//
// During emission, forward jumps cannot be resolved immediately because the
// target instruction hasn't been emitted yet. We record a relocation entry
// and patch it after all instructions are emitted.
// ─────────────────────────────────────────────────────────────────────────────

/// A single relocation entry for a forward jump.
#[derive(Debug, Clone)]
pub struct HotaruReloc {
    /// Offset in the code buffer of the 4-byte rel32 placeholder.
    pub patch_offset: usize,
    /// The bytecode IP that this jump should resolve to.
    pub target_bc_ip: usize,
}

/// Patch all relocation entries given the final bytecode-IP-to-code-offset mapping.
pub fn patch_relocations(code: &mut [u8], relocs: &[HotaruReloc], bc_ip_to_code_offset: &[usize]) {
    for reloc in relocs {
        let target_code_offset = if reloc.target_bc_ip < bc_ip_to_code_offset.len() {
            bc_ip_to_code_offset[reloc.target_bc_ip]
        } else {
            // Target is past the end of the function — point to the epilogue
            // which is the last entry in the offset map
            *bc_ip_to_code_offset.last().unwrap_or(&0)
        };
        // rel32 = target - (patch_site + 4)
        let rel32 = (target_code_offset as i64 - (reloc.patch_offset as i64 + 4)) as i32;
        let bytes = rel32.to_le_bytes();
        code[reloc.patch_offset..reloc.patch_offset + 4].copy_from_slice(&bytes);
    }
}
