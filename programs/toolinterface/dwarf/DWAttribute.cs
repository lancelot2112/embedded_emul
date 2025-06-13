using EmbedEmul.Types;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Binary;
using EmbedEmul.Memory;

namespace EmbedEmul.Programs.TIS.Dwarf
{
    public struct DWAttribute
    {
        internal static DWAttribute Zero = new DWAttribute(0, 0);
        internal DWAttrType _typeCode;
        public DWAttrType TypeCode { get { return _typeCode; } }
        internal DWForm _formCode;
        public DWForm FormCode { get { return _formCode; } }
        internal UInt64 _location;

        public DWAttribute(DWAttrType type, DWForm form)
        {
            //DWARF ver 2-4
            _typeCode = type;
            _formCode = form;
            _location = 0;

            //_form = ParseForm(info, strTable);
            //SeekEnd(info);
        }

        public DWAttribute(MemoryUnit info)
        {
            //DWARF ver 1.1
            UInt16 name = info.GetUInt16();
            _typeCode = (DWAttrType)((name & 0xfff0) >> 4);
            _formCode = (DWForm)(name & 0xf);
            _location = 0;

            //_form = ParseForm(bytes);
            //SeekEnd(info);
        }

        public void TakeInfo(UInt16 name)
        {
            _typeCode = (DWAttrType)((name & 0xfff0) >> 4);
            _formCode = (DWForm)(name & 0xf);
            _location = 0;
        }

        public Int64 Size(MemoryUnit info)
        {
            switch (_formCode)
            {
                case DWForm.DW_FORM_addr:
                    return 4; //size of address
                case DWForm.DW_FORM_block:
                    return (Int64)info.GetULEB128();
                case DWForm.DW_FORM_block1:
                    return info.GetUInt8();
                case DWForm.DW_FORM_block2:
                    return info.GetUInt16();
                case DWForm.DW_FORM_block4:
                    return info.GetUInt32();
                case DWForm.DW_FORM_exprloc:
                    throw new NotImplementedException();
                case DWForm.DW_FORM_data1:
                    return 1;
                case DWForm.DW_FORM_data2:
                    return 2;
                case DWForm.DW_FORM_data4:
                    return 4;
                case DWForm.DW_FORM_data8:
                    return 8;
                case DWForm.DW_FORM_sdata:
                    return -1; //Easiest just to consume dynamic len type
                case DWForm.DW_FORM_udata:
                    return -1;
                case DWForm.DW_FORM_flag:
                    return 1;
                case DWForm.DW_FORM_flag_present:
                    return 0;
                case DWForm.DW_FORM_reference:
                    return 4;
                case DWForm.DW_FORM_ref1:
                    return 1;
                case DWForm.DW_FORM_ref2:
                    return 2;
                case DWForm.DW_FORM_ref4:
                    return 4;
                case DWForm.DW_FORM_ref8:
                    return 8;
                case DWForm.DW_FORM_ref_addr:
                    return 4;
                case DWForm.DW_FORM_ref_sig8:
                    return 8;
                case DWForm.DW_FORM_ref_udata:
                    return -1;
                case DWForm.DW_FORM_strp:
                    return 4;
                case DWForm.DW_FORM_sec_offset:
                    return 4;
                case DWForm.DW_FORM_string:
                    return -1;
                case DWForm.DW_FORM_indirect:
                    throw new NotImplementedException();
                default: return -1;
            }
        }

        public void InitializeAndSkip(DWDie parent, MemoryUnit info, MemoryUnit strTab)
        {
            if (_formCode == 0)
                return;

            _location = (UInt64)info.CurrentAddress;
            if (parent != null )
            {
                if (_typeCode == DWAttrType.DW_AT_name)
                {
                    if (TryGetString(info, strTab, out parent._labelCache))
                        return;
                }
                else if (_typeCode == DWAttrType.DW_AT_data_location)
                {
                    if (TryGetString(info, strTab, out parent._linkLabelCache))
                        return;
                }
                else if (_typeCode == DWAttrType.DW_AT_member)
                {
                    ulong rtn;
                    if (TryGetUData(parent._cu, info, out rtn))
                    {
                        parent._memberOf = (UInt32)rtn;
                        return;
                    }
                }
                else if (_typeCode == DWAttrType.DW_AT_declaration)
                {
                    byte rtn;
                    if (TryGetFlag(info, out rtn))
                    {
                        parent._externalLink |= rtn == 1;
                        return;
                    }
                }
                else if (_typeCode == DWAttrType.DW_AT_sibling)
                {
                    ulong rtn;
                    if(TryGetUData(parent._cu, info, out rtn))
                    {
                        parent._sibling = (UInt32)rtn;
                        return;
                    }
                }

                //else if (TryGetTypeInfo(info, out parent._typeInfo))
                //    return;
            }

            Int64 size = Size(info);
            if (size > -1)
                info.SkipBytes(size);
            else
            {
                if (_formCode == DWForm.DW_FORM_ref_udata ||
                    _formCode == DWForm.DW_FORM_udata ||
                    _formCode == DWForm.DW_FORM_sdata)
                {
                     info.SkipLEB128();
                }
                else if (_formCode == DWForm.DW_FORM_string)
                    info.SkipString();
                else throw new NotImplementedException();
            }
        }

        public bool TryGetString(MemoryUnit info, MemoryUnit strTab, out string rtn)
        {
            info.CurrentAddress = _location;
            switch (_formCode)
            {
                case DWForm.DW_FORM_string:
                    {
                        rtn = info.GetString();
                    }
                    break;
                case DWForm.DW_FORM_strp:
                    {
                        UInt32 offset = info.GetUInt32();
                        if (strTab != null)
                        {
                            strTab.CurrentAddress = offset;
                            rtn = strTab.GetString();
                        }
                        else rtn = "";
                    }
                    break;
                default:
                    rtn = null;
                    return false;
            }
            return true;
        }

        public bool TrySetBlockWorkingRange(MemoryUnit block, out Int32 syncId)
        {
            UInt64 len;
            block.CurrentAddress = _location;
            switch (_formCode)
            {
                case DWForm.DW_FORM_block:
                    len = block.GetULEB128();
                    syncId = block.CacheRangeLength((Int64)len);
                    break;
                case DWForm.DW_FORM_block1:
                    len = block.GetUInt8();
                    syncId = block.CacheRangeLength((Int64)len);
                    break;
                case DWForm.DW_FORM_block2:
                    len = block.GetUInt16();
                    syncId = block.CacheRangeLength((Int64)len);
                    break;
                case DWForm.DW_FORM_block4:
                    len = block.GetUInt32();
                    syncId = block.CacheRangeLength((Int64)len);
                    break;
                default:
                    syncId = -1;
                    return false;
            }
            return true;
        }

        public bool TryGetTypeInfo(DWCompilationUnitHeader cu, MemoryUnit block, out DWTypeInfo rtn)
        {
            rtn = null;
            if (_typeCode == DWAttrType.DW_AT_mod_fund_type ||
                _typeCode == DWAttrType.DW_AT_mod_u_d_type ||
                _typeCode == DWAttrType.DW_AT_fund_type ||
                _typeCode == DWAttrType.DW_AT_user_def_type ||
                _typeCode == DWAttrType.DW_AT_type ||
                _typeCode == DWAttrType.DW_AT_base_types)
            {
                rtn = new DWTypeInfo(cu, block, this);
            }
            return rtn != null;
        }

        public bool TryGetUData(DWCompilationUnitHeader cu, MemoryUnit block, out UInt64 rtn)
        {
            block.CurrentAddress = _location;
            switch (_formCode)
            {
                case DWForm.DW_FORM_data1:
                    rtn = block.GetUInt8();
                    break;
                case DWForm.DW_FORM_data2:
                    rtn = block.GetUInt16();
                    break;
                case DWForm.DW_FORM_data4:
                    rtn = block.GetUInt32();
                    break;
                case DWForm.DW_FORM_data8:
                    rtn = block.GetUInt64();
                    break;
                case DWForm.DW_FORM_udata:
                    rtn = block.GetULEB128();
                    break;
                case DWForm.DW_FORM_reference:
                    rtn = block.GetUInt32();
                    break;
                case DWForm.DW_FORM_ref1:
                    rtn = block.GetUInt8() + cu._start;
                    break;
                case DWForm.DW_FORM_ref2:
                    rtn = block.GetUInt16() + cu._start;
                    break;
                case DWForm.DW_FORM_ref4:
                    rtn = block.GetUInt32() + cu._start;
                    break;
                case DWForm.DW_FORM_ref8:
                    rtn = block.GetUInt64() + cu._start;
                    break;
                case DWForm.DW_FORM_ref_udata:
                    rtn = block.GetULEB128() + cu._start;
                    break;
                //Reference refers to location inside Debug_Info section
                case DWForm.DW_FORM_ref_addr:
                    rtn = block.GetUInt32();
                    break;
                //Reference refers to location inside Type Unit
                case DWForm.DW_FORM_ref_sig8:
                    rtn = block.GetUInt64();
                    break;
                case DWForm.DW_FORM_strp:
                    rtn = block.GetUInt32();
                    break;
                case DWForm.DW_FORM_sec_offset:
                    rtn = block.GetUInt32();
                    break;
                case DWForm.DW_FORM_addr:
                    rtn = block.GetUInt32();
                    break;
                default:
                    rtn = 0;
                    return false;
            }
            return true;
        }

        public bool TryGetSData(MemoryUnit block, out Int64 rtn)
        {
            block.CurrentAddress = _location;
            switch (_formCode)
            {
                case DWForm.DW_FORM_data1:
                    rtn = block.GetInt8();
                    break;
                case DWForm.DW_FORM_data2:
                    rtn = block.GetInt16();
                    break;
                case DWForm.DW_FORM_data4:
                    rtn = block.GetInt32();
                    break;
                case DWForm.DW_FORM_data8:
                    rtn = block.GetInt64();
                    break;
                case DWForm.DW_FORM_sdata:
                    rtn = block.GetSLEB128();
                    break;
                default:
                    rtn = 0;
                    return false;
            }
            return true;
        }

        static HashSet<DWAttrType> __ValidExpressionAttr__ = new HashSet<DWAttrType>()
        {
            DWAttrType.DW_AT_location,
            DWAttrType.DW_AT_string_length,
            DWAttrType.DW_AT_byte_size,
            DWAttrType.DW_AT_bit_offset,
            DWAttrType.DW_AT_bit_size,
            DWAttrType.DW_AT_lower_bound,
            DWAttrType.DW_AT_return_addr,
            DWAttrType.DW_AT_bit_stride,
            DWAttrType.DW_AT_upper_bound,
            DWAttrType.DW_AT_count,
            DWAttrType.DW_AT_data_member_location,
            DWAttrType.DW_AT_frame_base,
            DWAttrType.DW_AT_segment,
            DWAttrType.DW_AT_static_link,
            DWAttrType.DW_AT_use_location,
            DWAttrType.DW_AT_vtable_elem_location,
            DWAttrType.DW_AT_allocated,
            DWAttrType.DW_AT_associated,
            DWAttrType.DW_AT_data_location,
            DWAttrType.DW_AT_byte_stride
        };
        public bool TryGetExpression(MemoryUnit block, DWDebug context, out DWExpression rtn)
        {
            if (__ValidExpressionAttr__.Contains(_typeCode))
            {
                Int32 syncId;
                block.CurrentAddress = _location;
                if (_formCode == DWForm.DW_FORM_sec_offset)
                {
                    context._debugLoc.CurrentAddress = block.GetUInt32();
                    throw new NotImplementedException();
                }
                else if (TrySetBlockWorkingRange(block, out syncId))
                {
                    rtn = new DWExpression(block);
                    block.DesyncCacheRange(syncId);
                }
                else
                    rtn = null;
            }
            else rtn = null;
            return rtn != null;
        }

        public bool TryGetFlag(MemoryUnit block, out byte rtn)
        {
            switch (_formCode)
            {
                case DWForm.DW_FORM_flag:
                    block.CurrentAddress = _location;
                    rtn = block.GetUInt8();
                    break;
                case DWForm.DW_FORM_flag_present:
                    rtn = 1;
                    break;
                default:
                    rtn = 0;
                    return false;
            }
            return true;

        }


        public override string ToString()
        {
            return string.Format("<{0:X}>{1,29}",_location,TypeCode);
        }
    }

    public class DWTypeInfo
    {
        internal List<DWModified> _modifiers;
        internal UInt64 _value;
        public UInt64 Value { get { return _value; } }
        internal DWAttribute _attr;

        public DWTypeInfo(DWCompilationUnitHeader cu, MemoryUnit block, DWAttribute attr)
        {
            _attr = attr;
            Int32 syncId, parentSyncId;
            if (attr._typeCode == DWAttrType.DW_AT_mod_fund_type)
            {
                parentSyncId = block._workingRange._id;
                if (attr.TrySetBlockWorkingRange(block, out syncId))
                {
                    block.CacheRangeExpand(-2);
                    _modifiers = GetModifiers(block).ToList();
                    _value = block.GetUInt16();
                    block.DesyncCacheRange(syncId, parentSyncId, true);
                }
                else throw new Exception();
            }
            else if (attr._typeCode == DWAttrType.DW_AT_mod_u_d_type)
            {
                parentSyncId = block._workingRange._id;
                if (attr.TrySetBlockWorkingRange(block, out syncId))
                {
                    block.CacheRangeExpand(-4);
                    _modifiers = GetModifiers(block).ToList();
                    _value = block.GetUInt32();
                    block.DesyncCacheRange(syncId, parentSyncId, true);
                }
                else throw new Exception();
            }
            else if (attr._typeCode == DWAttrType.DW_AT_fund_type ||
                     attr._typeCode == DWAttrType.DW_AT_user_def_type ||
                     attr._typeCode == DWAttrType.DW_AT_type ||
                     attr._typeCode == DWAttrType.DW_AT_base_types)
                attr.TryGetUData(cu, block, out _value);
            else throw new NotImplementedException();
        }

        public GenType ApplyModifiers(GenType original)
        {
            if (_modifiers != null)
            {
                foreach (DWModified modifier in _modifiers)
                    if (modifier == DWModified.DW_MOD_pointer_to)
                        original = new GenPointer(original);
            }
            return original;
        }
        private IEnumerable<DWModified> GetModifiers(MemoryUnit block)
        {
            while (!block.EndOfRange)
                yield return (DWModified)block.GetUInt8();
        }

        public override string ToString()
        {
            StringBuilder build = new StringBuilder();
            if (_modifiers != null)
            {
                foreach (DWModified modifier in _modifiers)
                {
                    build.Append(modifier);
                    build.Append(" ");
                }
            }
            if (_attr._typeCode == DWAttrType.DW_AT_mod_fund_type || _attr._typeCode == DWAttrType.DW_AT_fund_type)
                build.Append((DWFundamentalType)_value);
            else
                build.Append(_value.ToString("X"));

            return build.ToString();
        }
    }
}