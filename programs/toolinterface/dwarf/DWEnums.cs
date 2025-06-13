using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EmbedEmul.Programs.TIS.Dwarf
{
    public enum DWTag : ushort
    {
        DW_TAG_padding = 0x0000,
        DW_TAG_array_type = 0x0001,
        DW_TAG_class_type = 0x0002,
        DW_TAG_entry_point = 0x0003,
        DW_TAG_enumeration_type = 0x0004,
        DW_TAG_formal_parameter = 0x0005,
        DW_TAG_global_subroutine = 0x0006,
        DW_TAG_global_variable = 0x0007,
        DW_TAG_imported_declaration = 0x0008,
        DW_TAG_label = 0x000a,
        DW_TAG_lexical_block = 0x000b,
        DW_TAG_local_variable = 0x000c,
        DW_TAG_member = 0x000d,
        DW_TAG_pointer_type = 0x000f,
        DW_TAG_reference_type = 0x0010,
        //DW_TAG_source_file = 0x0011,
        DW_TAG_compile_unit = 0x0011,
        DW_TAG_string_type = 0x0012,
        DW_TAG_structure_type = 0x0013,
        DW_TAG_subroutine = 0x0014,
        DW_TAG_subroutine_type = 0x0015,
        DW_TAG_typedef = 0x0016,
        DW_TAG_union_type = 0x0017,
        DW_TAG_unspecified_parameters = 0x0018,
        DW_TAG_variant = 0x0019,
        DW_TAG_common_block = 0x001a,
        DW_TAG_common_inclusion = 0x001b,
        DW_TAG_inheritance = 0x001c,
        DW_TAG_inlined_subroutine = 0x001d,
        DW_TAG_module = 0x001e,
        DW_TAG_ptr_to_member_type = 0x001f,
        DW_TAG_set_type = 0x0020,
        DW_TAG_subrange_type = 0x0021,
        DW_TAG_with_stmt = 0x0022,
        DW_TAG_access_declaration = 0x0023,  //Start of DWARF 2.0
        DW_TAG_base_type = 0x0024,
        DW_TAG_catch_block = 0x0025,
        DW_TAG_const_type = 0x0026,
        DW_TAG_constant = 0x0027,
        DW_TAG_enumerator = 0x0028,
        DW_TAG_file_type = 0x0029,
        DW_TAG_friend = 0x002a,
        DW_TAG_namelist = 0x002b,
        DW_TAG_namelist_item = 0x002c,
        DW_TAG_packed_type = 0x002d,
        DW_TAG_subprogram = 0x002e,
        DW_TAG_template_type_parameter = 0x002f,
        DW_TAG_template_value_parameter = 0x0030,
        DW_TAG_thrown_type = 0x0031,
        DW_TAG_try_block = 0x0032,
        DW_TAG_variant_part = 0x0033,
        DW_TAG_variable = 0x0034,
        DW_TAG_volatile_type = 0x0035,
        DW_TAG_dwarf_procedure = 0x0036, //Start of DWARF3.0
        DW_TAG_restrict_type = 0x0037,
        DW_TAG_interface_type = 0x0038,
        DW_TAG_namespace = 0x0039,
        DW_TAG_imported_module = 0x003a,
        DW_TAG_unspecified_type = 0x003b,
        DW_TAG_partial_unit = 0x003c,
        DW_TAG_imported_unit = 0x003d,
        DW_TAG_condition = 0x003f,
        DW_TAG_shared_type = 0x0040,
        DW_TAG_type_unit = 0x0041, //Start of DWARF4.0
        DW_TAG_rvalue_reference_type = 0x0042,
        DW_TAG_template_alias = 0x0043
        //0x4080-0xffff User Defined
    }
    public enum DWChildren : byte //Introduced DWARF2.0
    {
        DW_CHILDREN_no = 0x0,
        DW_CHILDREN_yes = 0x1
    }

    public enum DWAttrType : ushort
    {
        DW_AT_sibling = 0x0001,
        DW_AT_location = 0x0002,
        DW_AT_name = 0x0003,
        DW_AT_fund_type = 0x0005,
        DW_AT_mod_fund_type = 0x0006,
        DW_AT_user_def_type = 0x0007,
        DW_AT_mod_u_d_type = 0x0008,
        DW_AT_ordering = 0x0009,
        DW_AT_subscr_data = 0x000a,
        DW_AT_byte_size = 0x000b,
        DW_AT_bit_offset = 0x000c,
        DW_AT_bit_size = 0x000d,
        DW_AT_element_list = 0x000f,
        DW_AT_stmt_list = 0x0010, //Pointer into .line or .debug_line
        DW_AT_low_pc = 0x0011,
        DW_AT_high_pc = 0x0012,
        DW_AT_language = 0x0013,
        DW_AT_member = 0x0014,
        DW_AT_discr = 0x15,
        DW_AT_discr_value = 0x16,
        DW_AT_visibility = 0x17,
        DW_AT_import = 0x18,
        DW_AT_string_length = 0x0019,
        DW_AT_common_reference = 0x001a,
        DW_AT_comp_dir = 0x1b,
        DW_AT_const_value = 0x001c,
        DW_AT_containing_type = 0x1d,
        DW_AT_default_value = 0x1e,
        DW_AT_friends = 0x1f,
        DW_AT_inline = 0x0020,
        DW_AT_is_optional = 0x0021,
        DW_AT_lower_bound = 0x0022,
        DW_AT_program = 0x0023,
        DW_AT_private = 0x0024,
        DW_AT_producer = 0x0025,
        DW_AT_protected = 0x0026,
        DW_AT_prototyped = 0x0027,
        DW_AT_public = 0x0028,
        DW_AT_pure_virtual = 0x0029,
        DW_AT_return_addr = 0x002a,
        DW_AT_specification = 0x002b,
        DW_AT_start_scope = 0x002c,
        DW_AT_bit_stride = 0x002e,
        DW_AT_upper_bound = 0x002f,
        DW_AT_virtual = 0x0030,
        DW_AT_abstract_origin = 0x0031,  //Start of DWARF2.0
        DW_AT_accesibility = 0x0032,
        DW_AT_address_class = 0x0033,
        DW_AT_artificial = 0x0034,
        DW_AT_base_types = 0x0035,
        DW_AT_calling_convention = 0x0036,
        DW_AT_count = 0x0037,
        DW_AT_data_member_location = 0x0038,
        DW_AT_decl_column = 0x0039,
        DW_AT_decl_file = 0x003a,
        DW_AT_decl_line = 0x003b,
        DW_AT_declaration = 0x003c,
        DW_AT_discr_list = 0x003d,
        DW_AT_encoding = 0x003e,
        DW_AT_external = 0x003f,
        DW_AT_frame_base = 0x0040,
        DW_AT_friend = 0x0041,
        DW_AT_identifier_case = 0x0042,
        DW_AT_macro_info = 0x0043,
        DW_AT_namelist_item = 0x0044,
        DW_AT_priority = 0x0045,
        DW_AT_segment = 0x0046,
        DW_AT_specificiation = 0x0047,
        DW_AT_static_link = 0x0048,
        DW_AT_type = 0x0049,
        DW_AT_use_location = 0x004a,
        DW_AT_variable_parameter = 0x004b,
        DW_AT_virtuality = 0x004c,
        DW_AT_vtable_elem_location = 0x004d,
        DW_AT_allocated = 0x004e, //Start of DWARF3.0
        DW_AT_associated = 0x004f,
        DW_AT_data_location = 0x0050,
        DW_AT_byte_stride = 0x0051,
        DW_AT_entry_pc = 0x0052,
        DW_AT_use_UTF8 = 0x0053,
        DW_AT_extension = 0x0054,
        DW_AT_ranges = 0x0055,
        DW_AT_trampoline = 0x0056,
        DW_AT_call_column = 0x0057,
        DW_AT_call_file = 0x0058,
        DW_AT_call_line = 0x0059,
        DW_AT_description = 0x005a,
        DW_AT_binary_scale = 0x005b,
        DW_AT_decimal_scale = 0x005c,
        DW_AT_small = 0x005d,
        DW_AT_decimal_sign = 0x005e,
        DW_AT_digit_count = 0x005f,
        DW_AT_picture_string = 0x0060,
        DW_AT_mutable = 0x0061,
        DW_AT_threads_scaled = 0x0062,
        DW_AT_explicit = 0x0063,
        DW_AT_object_pointer = 0x0064,
        DW_AT_endianity = 0x0065,
        DW_AT_elemental = 0x0066,
        DW_AT_pure = 0x0067,
        DW_AT_recursive = 0x0068,
        DW_AT_signature = 0x0069, //Start of DWARF4.0
        DW_AT_main_subprogram = 0x006a,
        DW_AT_data_bit_offset = 0x006b,
        DW_AT_const_expr = 0x006c,
        DW_AT_enum_class = 0x006d,
        DW_AT_linkage_name = 0x006e,
        DW_AT_noreturn = 0x0087,
        DW_AT_MIPS_fde = 0x2001,
        DW_AT_MIPS_loop_begin = 0x2002,
        DW_AT_MIPS_tail_loop_begin = 0x2003,
        DW_AT_MIPS_epilog_begin = 0x2004,
        DW_AT_MIPS_loop_unroll_factor = 0x2005,
        DW_AT_MIPS_software_pipeline_depth = 0x2006,
        DW_AT_MIPS_linkage_name = 0x2007,
        DW_AT_MIPS_stride = 0x2008,
        DW_AT_MIPS_abstract_name = 0x2009,
        DW_AT_MIPS_clone_origin = 0x200a,
        DW_AT_MIPS_has_inlines = 0x200b,
        DW_AT_MIPS_stride_byte = 0x200c,
        DW_AT_MIPS_stride_elem = 0x200d,
        DW_AT_MIPS_ptr_dopetype = 0x200e,
        DW_AT_MIPS_allocatable_dopetype = 0x200f,
        DW_AT_MIPS_assumed_shape_dopetype = 0x2010,
        DW_AT_MIPS_assumed_size = 0x2011,
        DW_AT_sf_names = 0x2101,
        DW_AT_src_info = 0x2102,
        DW_AT_mac_info = 0x2103,
        DW_AT_src_coords = 0x2104,
        DW_AT_body_begin = 0x2105,
        DW_AT_body_end = 0x2106,
        DW_AT_GNU_vector = 0x2107,
        DW_AT_GNU_guarded_by = 0x2108,
        DW_AT_GNU_pt_guarded_by = 0x2109,
        DW_AT_GNU_guarded = 0x210a,
        DW_AT_GNU_pt_guarded = 0x210b,
        DW_AT_GNU_locks_excluded = 0x210c,
        DW_AT_GNU_exclusive_locks_required = 0x210d,
        DW_AT_GNU_shared_locks_required = 0x210e,
        DW_AT_GNU_odr_signature = 0x210f,
        DW_AT_GNU_template_name = 0x2110,
        DW_AT_GNU_call_site_value = 0x2111,
        DW_AT_GNU_call_site_data_value = 0x2112,
        DW_AT_GNU_call_site_target = 0x2113,
        DW_AT_GNU_call_site_target_clobbered = 0x2114,
        DW_AT_GNU_tail_call = 0x2115,
        DW_AT_GNU_all_tail_call_sites = 0x2116,
        DW_AT_GNU_all_call_sites = 0x2117,
        DW_AT_GNU_all_source_call_sites = 0x2118,
        DW_AT_GNU_macros = 0x2119,
        DW_AT_GNU_deleted = 0x211a,
        DW_AT_GNU_dwo_name = 0x2130,
        DW_AT_GNU_dwo_id = 0x2131,
        DW_AT_GNU_ranges_base = 0x2132,
        DW_AT_GNU_addr_base = 0x2133,
        DW_AT_GNU_pubnames = 0x2134,
        DW_AT_GNU_pubtypes = 0x2135
        //0x2000-0x3fff User Defined
    }

    public enum DWForm : byte
    {
        DW_FORM_addr = 0x01, //Object of appropriate size to hold an address on target machine
        DW_FORM_reference = 0x02, //4-byte value defn. offset from start of .debug section
        DW_FORM_block2 = 0x03, //2-byte length followed by information bytes
        DW_FORM_block4 = 0x04, //4-byte length followed by information bytes
        DW_FORM_data2 = 0x05, //2-byte unsigned value (half)
        DW_FORM_data4 = 0x06, //4-byte unsigned value (word)
        DW_FORM_data8 = 0x07, //8-byte unsigned value (doubleword)
        DW_FORM_string = 0x08,  //Null terminated char array
        DW_FORM_block = 0x09, //Start of DWARF2.0 //unsigned LEB128 length followed by the number of bytes specified by the length
        DW_FORM_block1 = 0x0a,  //1-byte len followed by 0-255 bytes
        DW_FORM_data1 = 0x0b,
        DW_FORM_flag = 0x0c, //1-byte
        DW_FORM_sdata = 0x0d,
        DW_FORM_strp = 0x0e, //offset in .debug_str
        DW_FORM_udata = 0x0f,
        DW_FORM_ref_addr = 0x10, //offset in .debug_info
        DW_FORM_ref1 = 0x11,
        DW_FORM_ref2 = 0x12,
        DW_FORM_ref4 = 0x13,
        DW_FORM_ref8 = 0x14,
        DW_FORM_ref_udata = 0x15,
        DW_FORM_indirect = 0x16,
        DW_FORM_sec_offset = 0x17, //Start of DWARF4.0,
        DW_FORM_exprloc = 0x18,
        DW_FORM_flag_present = 0x19,
        DW_FORM_ref_sig8 = 0x20,
        //DW_FORM_GNU_addr_index = 0x1f01,
        //DW_FORM_GNU_str_index = 0x1f02,
        //DW_FORM_GNU_ref_alt = 0x1f20,
        //DW_FORM_GNU_strp_alt = 0x1f21
    }

    public enum DWOpType : byte
    {
        DW_OP_reg = 0x01,     //constant address
        DW_OP_breg = 0x02,
        DW_OP_addr = 0x03,
        DW_OP_const = 0x04,
        DW_OP_deref2 = 0x05,
        DW_OP_deref = 0x06,
        //DW_OP_deref4 = 0x06,
        DW_OP_add = 0x07,
        DW_OP_const1u = 0x08, //Start of DWARF 2.0 // 1 byte constant
        DW_OP_const1s = 0x09,
        DW_OP_const2u = 0x0a, //2 byte constant
        DW_OP_const2s = 0x0b,
        DW_OP_const4u = 0x0c, //4 byte constant
        DW_OP_const4s = 0x0d,
        DW_OP_const8u = 0x0e, //8 byte constant
        DW_OP_const8s = 0x0f,
        DW_OP_constu = 0x10, //ULEB128 Constant
        DW_OP_consts = 0x11,   //SLEB128 Constant
        DW_OP_dup = 0x12,
        DW_OP_drop = 0x13,
        DW_OP_over = 0x14,
        DW_OP_pick = 0x15,
        DW_OP_swap = 0x16,
        DW_OP_rot = 0x17,
        DW_OP_xderef = 0x18,
        DW_OP_abs = 0x19,
        DW_OP_and = 0x1a,
        DW_OP_div = 0x1b,
        DW_OP_minus = 0x1c,
        DW_OP_mod = 0x1d,
        DW_OP_mul = 0x1e,
        DW_OP_neg = 0x1f,
        DW_OP_not = 0x20,
        DW_OP_or = 0x21,
        DW_OP_plus = 0x22,
        DW_OP_plus_uconst = 0x23, //ULEB128 Addend
        DW_OP_shl = 0x24,
        DW_OP_shr = 0x25,
        DW_OP_shra = 0x26,
        DW_OP_xor = 0x27,
        DW_OP_skip = 0x2f,
        DW_OP_bra = 0x28,
        DW_OP_eq = 0x29,
        DW_OP_ge = 0x2a,
        DW_OP_gt = 0x2b,
        DW_OP_le = 0x2c,
        DW_OP_lt = 0x2d,
        DW_OP_ne = 0x2e,
        DW_OP_lit0 = 0x30,
        DW_OP_lit1 = 0x31,
        DW_OP_lit2 = 0x32,
        DW_OP_lit3 = 0x33,
        DW_OP_lit4 = 0x34,
        DW_OP_lit5 = 0x35,
        DW_OP_lit6 = 0x36,
        DW_OP_lit7 = 0x37,
        DW_OP_lit8 = 0x38,
        DW_OP_lit9 = 0x39,
        DW_OP_lit10 = 0x3a,
        DW_OP_lit11 = 0x3b,
        DW_OP_lit12 = 0x3c,
        DW_OP_lit13 = 0x3d,
        DW_OP_lit14 = 0x3e,
        DW_OP_lit15 = 0x3f,
        DW_OP_lit16 = 0x40,
        DW_OP_lit17 = 0x41,
        DW_OP_lit18 = 0x42,
        DW_OP_lit19 = 0x43,
        DW_OP_lit20 = 0x44,
        DW_OP_lit21 = 0x45,
        DW_OP_lit22 = 0x46,
        DW_OP_lit23 = 0x47,
        DW_OP_lit24 = 0x48,
        DW_OP_lit25 = 0x49,
        DW_OP_lit26 = 0x4a,
        DW_OP_lit27 = 0x4b,
        DW_OP_lit28 = 0x4c,
        DW_OP_lit29 = 0x4d,
        DW_OP_lit30 = 0x4e,
        DW_OP_lit31 = 0x4e,
        DW_OP_reg0 = 0x50,
        DW_OP_reg1 = 0x51,
        DW_OP_reg2 = 0x52,
        DW_OP_reg3 = 0x53,
        DW_OP_reg4 = 0x54,
        DW_OP_reg5 = 0x55,
        DW_OP_reg6 = 0x56,
        DW_OP_reg7 = 0x57,
        DW_OP_reg8 = 0x58,
        DW_OP_reg9 = 0x59,
        DW_OP_reg10 = 0x5a,
        DW_OP_reg11 = 0x5b,
        DW_OP_reg12 = 0x5c,
        DW_OP_reg13 = 0x5d,
        DW_OP_reg14 = 0x5e,
        DW_OP_reg15 = 0x5f,
        DW_OP_reg16 = 0x60,
        DW_OP_reg17 = 0x61,
        DW_OP_reg18 = 0x62,
        DW_OP_reg19 = 0x63,
        DW_OP_reg20 = 0x64,
        DW_OP_reg21 = 0x65,
        DW_OP_reg22 = 0x66,
        DW_OP_reg23 = 0x67,
        DW_OP_reg24 = 0x68,
        DW_OP_reg25 = 0x69,
        DW_OP_reg26 = 0x6a,
        DW_OP_reg27 = 0x6b,
        DW_OP_reg28 = 0x6c,
        DW_OP_reg29 = 0x6d,
        DW_OP_reg30 = 0x6e,
        DW_OP_reg31 = 0x6f,
        DW_OP_breg0 = 0x70,
        DW_OP_breg1 = 0x71,
        DW_OP_breg2 = 0x72,
        DW_OP_breg3 = 0x73,
        DW_OP_breg4 = 0x74,
        DW_OP_breg5 = 0x75,
        DW_OP_breg6 = 0x76,
        DW_OP_breg7 = 0x77,
        DW_OP_breg8 = 0x78,
        DW_OP_breg9 = 0x79,
        DW_OP_breg10 = 0x7a,
        DW_OP_breg11 = 0x7b,
        DW_OP_breg12 = 0x7c,
        DW_OP_breg13 = 0x7d,
        DW_OP_breg14 = 0x7e,
        DW_OP_breg15 = 0x7f,
        DW_OP_breg16 = 0x80,
        DW_OP_breg17 = 0x81,
        DW_OP_breg18 = 0x82,
        DW_OP_breg19 = 0x83,
        DW_OP_breg20 = 0x84,
        DW_OP_breg21 = 0x85,
        DW_OP_breg22 = 0x86,
        DW_OP_breg23 = 0x87,
        DW_OP_breg24 = 0x88,
        DW_OP_breg25 = 0x89,
        DW_OP_breg26 = 0x8a,
        DW_OP_breg27 = 0x8b,
        DW_OP_breg28 = 0x8c,
        DW_OP_breg29 = 0x8d,
        DW_OP_breg30 = 0x8e,
        DW_OP_breg31 = 0x8f,
        DW_OP_regx = 0x90,
        DW_OP_fbreg = 0x91,
        DW_OP_bregx = 0x92,
        DW_OP_piece = 0x93,
        DW_OP_deref_size = 0x94,
        DW_OP_xderef_size = 0x95,
        DW_OP_nop = 0x96,
        DW_OP_push_object_address = 0x97, //Start of DWARF3.0
        DW_OP_call2 = 0x98,
        DW_OP_call4 = 0x99,
        DW_OP_call_ref = 0x9a,
        DW_OP_form_tls_address = 0x9b,
        DW_OP_call_frame_cfa = 0x9c,
        DW_OP_bit_piece = 0x9d,
        DW_OP_implicit_value = 0x9e, //Start of DWARF4.0
        DW_OP_stack_value = 0x9f,
        DW_OP_GNU_push_tls_address = 0xe0,
        DW_OP_GNU_uninit = 0xf0,
        DW_OP_GNU_encoded_addr = 0xf1,
        DW_OP_GNU_implicit_pointer = 0xf2,
        DW_OP_GNU_entry_value = 0xf3,
        DW_OP_GNU_const_type = 0xf4,
        DW_OP_GNU_regval_type = 0xf5,
        DW_OP_GNU_deref_type = 0xf6,
        DW_OP_GNU_convert = 0xf7,
        DW_OP_GNU_reinterpret = 0xf9,
        DW_OP_GNU_parameter_ref = 0xfa,
        DW_OP_GNU_addr_index = 0xfb,
        DW_OP_GNU_const_index = 0xfc
        //0xe0-0xff User Defined
    }

    public enum DWBaseType : byte //Introduced DWARF2.0
    {
        DW_ATE_address = 0x01,
        DW_ATE_boolean = 0x02,
        DW_ATE_complex_float = 0x03,
        DW_ATE_float = 0x04,
        DW_ATE_signed = 0x05,
        DW_ATE_signed_char = 0x06,
        DW_ATE_unsigned = 0x07,
        DW_ATE_unsigned_char = 0x08,
        DW_ATE_imaginary_float = 0x09, //Start of DWARF3.0
        DW_ATE_packed_decimal = 0x0a,
        DW_ATE_numeric_string = 0x0b,
        DW_ATE_edited = 0x0c,
        DW_ATE_signed_fixed = 0x0d,
        DW_ATE_unsigned_fixed = 0x0e,
        DW_ATE_decimal_float = 0x0f,
        DW_ATE_UTF = 0x10
        //0x80-0xff User Defined
    }

    public enum DWDecimalSign : byte //Introduced DWARF3.0
    {
        DW_DS_unsigned = 0x01,
        DW_DS_leading_overpunch = 0x02,
        DW_DS_trailing_overpunch = 0x03,
        DW_DS_leading_separate = 0x04,
        DW_DS_trailing_separate = 0x05,
    }

    public enum DWEndian : byte //Introduced DWARF3.0 TODO: Rectify with ByteOrder enum
    {
        DW_END_default = 0x00,
        DW_END_big = 0x01,
        DW_END_little = 0x02
        //0x40-0xff User Defined
    }

    public enum DWAccessibility : byte //Introduced DWARF2.0
    {
        DW_ACCESS_public = 0x01,
        DW_ACCESS_protected = 0x02,
        DW_ACCESS_private = 0x03
    }

    public enum DWVisibility : byte //Introduced DWARF2.0
    {
        DW_VIS_local = 0x01,
        DW_VIS_exported = 0x02,
        DW_VIS_qualified = 0x03
    }

    public enum DWVirtuality : byte //Introduced DWARF2.0
    {
        DW_VIRTUALITY_none = 0x00,
        DW_VIRTUALITY_virtual = 0x01,
        DW_VIRTUALITY_pure_virtual = 0x02
    }

    public enum DWLanguage : uint
    {
        DW_LANG_C89 = 0x00000001,
        DW_LANG_C = 0x00000002,
        DW_LANG_Ada83 = 0x00000003,
        DW_LANG_C_plus_plus = 0x00000004,
        DW_LANG_Cobol74 = 0x00000005,
        DW_LANG_Cobol85 = 0x00000006,
        DW_LANG_Fortran77 = 0x00000007,
        DW_LANG_Fortran90 = 0x00000008,
        DW_LANG_Pascal83 = 0x00000009,
        DW_LANG_Modula2 = 0x0000000a,
        DW_LANG_Java = 0x0000000b, //Start of DWARF3.0
        DW_LANG_C99 = 0x0000000c,
        DW_LANG_Ada95 = 0x0000000d,
        DW_LANG_Fortran95 = 0x0000000e,
        DW_LANG_PLI = 0x0000000f,
        DW_LANG_ObjC = 0x00000010,
        DW_LANG_ObjC_plus_plus = 0x00000011,
        DW_LANG_UPC = 0x00000012,
        DW_LANG_D = 0x00000013,
        DW_LANG_Python = 0x00000014,
        DW_LANG_Go = 0x16,
        DW_LANG_C_plus_plus_11 = 0x1a,
        DW_LANG_C11 = 0x1d,
        DW_LANG_C_plus_plus_14 = 0x21,
        DW_LANG_Fortran03 = 0x22,
        DW_LANG_Fortran08 = 0x23,
        DW_LANG_MIPS_Assembly = 0x8001
        //0x8000-0xffff User
    }

    public enum DWIdentifierCase : byte //Introduced DWARF2.0
    {
        DW_ID_case_sensitive = 0x00,
        DW_ID_up_case = 0x01,
        DW_ID_down_case = 0x02,
        DW_ID_case_insensitive = 0x03
    }

    public enum DWCallingConvention : byte //Introduced DWARF2.0
    {
        DW_CC_normal = 0x01,
        DW_CC_program = 0x02,
        DW_CC_nocall = 0x03,
        //0x40-0xff User Defined
    }

    public enum DWInline : byte //Introduced DWARF2.0
    {
        DW_INL_not_inlined = 0x00,
        DW_INL_inlined = 0x01,
        DW_INL_declared_not_inlined = 0x02,
        DW_INL_declared_inlined = 0x03
    }

    /// <summary>
    /// Defines the ordering of arrays.  May be defined as language default in a 
    /// language DIE.
    /// </summary>
    public enum DWArrayOrdering
    {
        DW_ORD_row_major = 0x0,
        DW_ORD_col_major = 0x1
    }

    public enum DiscriminantListsCode : byte //Introduced DWARF2.0
    {
        DW_DSC_label = 0x00,
        DW_DSC_range = 0x01
    }

    public enum DWLineStandard : byte //Introduced DWARF2.0
    {
        DW_LNS_copy = 0x01,
        DW_LNS_advance_pc = 0x02,
        DW_LNS_advance_line = 0x03,
        DW_LNS_set_file = 0x04,
        DW_LNS_set_column = 0x05,
        DW_LNS_negate_stmt = 0x06,
        DW_LNS_set_basic_block = 0x07,
        DW_LNS_const_add_pc = 0x08,
        DW_LNS_fixed_advance_pc = 0x09,
        DW_LNS_set_prologue_end = 0x0a, //Start of DWARF3.0
        DW_LNS_set_epilogue_begin = 0x0b,
        DW_LNS_set_isa = 0x0c
    }

    public enum DWLineExtended : byte //Introduced DWARF2.0
    {
        DW_LNE_end_sequence = 0x01,
        DW_LNE_set_address = 0x02,
        DW_LNE_define_file = 0x03,
        DW_LNE_set_discriminator = 0x04 //Start of DWARF4.0
        //0x80-0xff User Defined
    }

    public enum DWMacroInformation : byte //Introduced DWARF2.0
    {
        DW_MACINFO_define = 0x01,
        DW_MACINFO_undef = 0x02,
        DW_MACINFO_start_file = 0x03,
        DW_MACINFO_end_file = 0x04,
        DW_MACINFO_vendor_ext = 0xff
    }

    public enum DWArraySubscriptFormat : byte
    {
        DW_FMT_FT_C_C = 0x0,
        DW_FMT_FT_C_X = 0x1,
        DW_FMT_FT_X_C = 0x2,
        DW_FMT_FT_X_X = 0x3,
        DW_FMT_UT_C_C = 0x4,
        DW_FMT_UT_C_X = 0x5,
        DW_FMT_UT_X_C = 0x6,
        DW_FMT_UT_X_X = 0x7,
        DW_FMT_ET = 0x8
    }

    /// <summary>
    /// Definitions for a DIE of a Modified Type (ie. ModifiedFundamentalType,
    /// ModifiedUserDefinedType)
    /// </summary>
    public enum DWModified : byte
    {
        DW_MOD_pointer_to = 0x01,
        DW_MOD_reference_to = 0x02,
        DW_MOD_const = 0x03,
        DW_MOD_volatile = 0x04
        //0x80-0xff User Defined
    }

    /// <summary>
    /// Definitions for a DIE of Fundamental Type
    /// </summary>
    public enum DWFundamentalType : ushort
    {
        DW_FT_char = 0x0001,
        DW_FT_signed_char = 0x0002,
        DW_FT_unsigned_char = 0x0003,
        DW_FT_short = 0x0004,
        DW_FT_signed_short = 0x0005,
        DW_FT_unsigned_short = 0x0006,
        DW_FT_integer = 0x0007,
        DW_FT_signed_integer = 0x0008,
        DW_FT_unsigned_integer = 0x0009,
        DW_FT_long = 0x000a,
        DW_FT_signed_long = 0x000b,
        DW_FT_unsigned_long = 0x000c,
        DW_FT_pointer = 0x000d,
        DW_FT_float = 0x000e,
        DW_FT_dbl_prec_float = 0x000f,
        DW_FT_ext_prec_float = 0x0010,
        DW_FT_complex = 0x0011,
        DW_FT_dbl_prec_complex = 0x0012,
        DW_FT_void = 0x0014,
        DW_FT_boolean = 0x0015,
        DW_FT_ext_prec_complex = 0x0016,
        DW_FT_label = 0x0017,
        //0x8000-0xffff User Defined
        DW_FT_signed_long_long = 0x8008,
        DW_FT_unsigned_long_long = 0x8208
    }

    public enum DWCallFrame
    {
        DW_CFA_MIPS_advance_loc8,
        DW_CFA_GNU_window_save = 0x2d,
        DW_CFA_GNU_args_size = 0x2e,
        DW_CFA_GNU_negative_offset_extended = 0x2f
    }
}
