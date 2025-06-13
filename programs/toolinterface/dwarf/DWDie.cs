using EmbedEmul.Elf;
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using EmbedEmul.Binary;
using EmbedEmul.Types;
using GenericUtilitiesLib;
using EmbedEmul.Memory;

namespace EmbedEmul.Programs.TIS.Dwarf
{
    /// <summary>
    /// Represented as a 4-byte (inclusive) length followed by a 2-byte tag and
    /// then a list of attributes.  DIE with (inclusive) length less than 8 is NULL entry
    /// </summary>
    public class DWDie : ICachedObject
    {
        public static ObjectCache<DWDie> Cache = new ObjectCache<DWDie>();
        public void Release()
        {
            Cache.ReleaseObject(this);
        }

        internal DWCompilationUnitHeader _cu;

        /// <summary>
        /// Offset from the start of the .debug section
        /// </summary>
        public UInt32 Offset { get { return _offset; } }
        internal UInt32 _offset;

        public UInt32 Sibling { get { return _sibling; } }
        internal UInt32 _sibling;

        /// <summary>
        /// Inclusive length of DIE in bytes.
        /// </summary>
        public UInt32 Length { get { return _length; } }
        internal UInt32 _length;

        /// <summary>
        /// Defines the type of DIE.
        /// </summary>
        public DWTag Tag { get { return _tag; } }
        internal DWTag _tag;

        //public DWChildren Children { get { return _children; } }
        internal DWChildren _children;

        public IEnumerable<DWAttribute> Attributes { get { if (_attributes == null) yield break; else for(int ii = 0; ii< _attributeCount; ii++) yield return _attributes[ii]; } }
        internal DWAttribute[] _attributes;
        internal byte _attributeCount;

        public string Label { get { return _labelCache != null ? _labelCache : ""; } }
        internal string _labelCache;

        public string LinkLabel { get { return _linkLabelCache != null ? _linkLabelCache : ""; } }
        internal string _linkLabelCache;

        public UInt32 MemberOf { get { return _memberOf; } }
        internal UInt32 _memberOf;

        public bool ExternalLink { get { return _externalLink; } }
        internal bool _externalLink;
        public Int64 TypeRef { get { return _typeInfo != null ? (Int64)_typeInfo._value: -1; } }
        internal DWTypeInfo _typeInfo;

        public bool TryGetAttribute(DWAttrType type, out DWAttribute attribute)
        {
            attribute = DWAttribute.Zero;
            if (_attributes != null)
            {
                for (int ii = 0; ii < _attributeCount; ii++)
                {
                    if (_attributes[ii]._typeCode == type)
                    {
                        attribute = _attributes[ii];
                        return true;
                    }
                }
            }
            return false;
        }

        #region GetString
        public bool TryGetString(DWAttrType code, out string rtn)
        {
            DWAttribute attr;
            if (code == DWAttrType.DW_AT_name && _labelCache != null)
            {
                rtn = _labelCache;
                return rtn.Length > 0;
            }
            else if (TryGetAttribute(code, out attr))
            {
                bool success = attr.TryGetString(_cu._debugInfo, _cu._context._debugStr, out rtn);
                if (code == DWAttrType.DW_AT_name)
                    _labelCache = rtn;
                return success;
            }
            rtn = "";
            return false;
        }
        public string GetString(DWAttrType code)
        {
            string rtn;
         if (TryGetString(code, out rtn))
            return rtn;
         else if (code == DWAttrType.DW_AT_name)
            return "anon" + _offset.ToString("X");
         else
            throw new NotSupportedException(string.Format("Operation not supported for attribute: {0}", code));
        }
        #endregion

        #region GetBlock

        public bool TryGetBlock(DWAttrType code, out MemoryUnit block, out Int32 syncId)
        {
            DWAttribute attr;
            block = _cu._debugInfo;
            if(TryGetAttribute(code, out attr))
                return attr.TrySetBlockWorkingRange(block, out syncId);
            block = null;
            syncId = -1;
            return false;

        }
        public MemoryUnit GetBlock(DWAttrType code, out Int32 syncId)
        {
            MemoryUnit block;
            if(TryGetBlock(code, out block, out syncId))
                return block;
            else throw new NotSupportedException(string.Format("Operation not supported for attribute: {0}", code));
        }
        #endregion

        #region GetTypeInfo

        public bool TryGetTypeInfo(DWAttrType code, out DWTypeInfo rtn)
        {
            DWAttribute attr;

            if (TryGetAttribute(code, out attr))
                return attr.TryGetTypeInfo(_cu, _cu._debugInfo, out rtn);
            rtn = null;
            return false;
        }
        #endregion

        #region GetElementList
        //TODO: Perhaps make a generic method to cast the raw byte stream to some list of a certain type
        public IEnumerable<ElementListEntry> GetElementList(DWAttrType code)
        {
            MemoryUnit block;
            Int32 syncId;
            if (TryGetBlock(code, out block, out syncId))
            {
                while (!block.EndOfRange)
                {
                    ElementListEntry entry = new ElementListEntry()
                    {
                        _value = block.GetUInt32(),
                        _label = block.GetString()
                    };
                    yield return entry;
                }
                block.DesyncCacheRange(syncId, passIndex: true);
            }
        }
        #endregion

        #region GetExpression
        public bool TryGetExpression(DWAttrType code, out DWExpression rtn)
        {
            DWAttribute attr;
            if (TryGetAttribute(code, out attr))
                return attr.TryGetExpression(_cu._debugInfo, _cu._context, out rtn);
            rtn = null;
            return false;

        }
        public DWExpression GetExpression(DWAttrType code)
        {
            DWExpression rtn;
            if(TryGetExpression(code, out rtn))
                return rtn;
            else throw new NotSupportedException(string.Format("Operation not supported for attribute: {0}", code));
        }
        #endregion

        #region GetFlag

        public bool TryGetFlag(DWAttrType code, out byte rtn)
        {
            DWAttribute attr;
            if (TryGetAttribute(code, out attr))
                return attr.TryGetFlag(_cu._debugInfo, out rtn);
            rtn = 0;
            return false;

        }
        public byte GetFlag(DWAttrType code)
        {
            byte rtn;
            if (TryGetFlag(code, out rtn))
                return rtn;
            else return 0; //TODO: Check for valid DWAttrType codes before allowing this to return
        }
        #endregion

        #region Dwarf Section Offset Form
        //public UInt64 RectifyReference(UInt64 value, DWForm formCode)
        //{
        //    if (_cu._context._dwarfVersion > 1)
        //    {
        //        if (formCode == DWForm.DW_FORM_ref1 ||
        //            formCode == DWForm.DW_FORM_ref2 ||
        //            formCode == DWForm.DW_FORM_ref4 ||
        //            formCode == DWForm.DW_FORM_ref8 ||
        //            formCode == DWForm.DW_FORM_ref_udata)
        //        {
        //            return value + _cu._start;
        //        }
        //    }
        //    return value;
        //}
        //public DwarfRange GetRangeByReference(DWAttribute code)
        //{
        //    DwarfAttribute attr;
        //    if (TryGetAttribute(code, out attr))
        //    {
        //        if (attr._typeCode == DWAttribute.DW_AT_start_scope ||
        //            attr._typeCode == DWAttribute.DW_AT_ranges)
        //        {
        //            _cu._debugInfo.Location = (long)attr._location;
        //            UInt32 offset = _cu._debugInfo.TakeUInt32();

        //        }
        //        else
        //            throw new NotSupportedException(string.Format("{0}:{1} not supported for reference data.", attr._typeCode, attr._formCode));
        //    }
        //    return DwarfRange;
        //}

        //public DwarfStmtList GetStmtListByReference(DWAttribute code)
        //{
        //    DwarfAttribute attr;
        //    if (TryGetAttribute(code, out attr))
        //    {
        //        if (attr._typeCode == DWAttribute.DW_AT_stmt_list)
        //        {
        //            _cu._debugInfo.Location = (long)attr._location;
        //            UInt32 offset = _cu._debugInfo.TakeUInt32();

        //        }
        //        else
        //            throw new NotSupportedException(string.Format("{0}:{1} not supported for reference data.", attr._typeCode, attr._formCode));
        //    }
        //    return DwarfStmtList;
        //}

        //public DwarfMacro GetMacroByReference(DWAttribute code)
        //{
        //    DwarfAttribute attr;
        //    if (TryGetAttribute(code, out attr))
        //    {
        //        if (attr._typeCode == DWAttribute.DW_AT_macro_info)
        //        {
        //            _cu._debugInfo.Location = (long)attr._location;
        //            UInt32 offset = _cu._debugInfo.TakeUInt32();

        //        }
        //        else
        //            throw new NotSupportedException(string.Format("{0}:{1} not supported for reference data.", attr._typeCode, attr._formCode));
        //    }
        //    return DwarfMacro;

        //}

        //public DwarfExpression GetExpressionByReference(DWAttribute code)
        //{
        //    DwarfAttribute attr;
        //    if (TryGetAttribute(code, out attr))
        //    {
        //        if (attr._typeCode == DWAttribute.DW_AT_location ||
        //               attr._typeCode == DWAttribute.DW_AT_string_length ||
        //               attr._typeCode == DWAttribute.DW_AT_return_addr ||
        //               attr._typeCode == DWAttribute.DW_AT_data_member_location ||
        //               attr._typeCode == DWAttribute.DW_AT_frame_base ||
        //               attr._typeCode == DWAttribute.DW_AT_segment ||
        //               attr._typeCode == DWAttribute.DW_AT_static_link ||
        //               attr._typeCode == DWAttribute.DW_AT_use_location ||
        //               attr._typeCode == DWAttribute.DW_AT_vtable_elem_location)
        //        {
        //            _cu._debugInfo.Location = (long)attr._location;
        //            UInt32 offset = _cu._debugInfo.TakeUInt32();

        //        }
        //        else
        //            throw new NotSupportedException(string.Format("{0}:{1} not supported for reference data.", attr._typeCode, attr._formCode));
        //    }
        //    return DwarfExpression;
        //}
        #endregion

        #region GetDIE
        public bool TryGetDIE(DWAttrType code, out DWDie rtn)
        {
            DWAttribute attr;
            if (TryGetAttribute(code, out attr))
            {
                UInt64 offset;
                if(attr.TryGetUData(_cu, _cu._debugInfo, out offset))
                {
                    //offset = RectifyReference(offset, attr._formCode);
                    rtn = _cu.GetDIE((uint)offset);
                    return true;
                }
            }
            rtn = null;
            return false;

        }
        public DWDie GetDIE(DWAttrType code)
        {
            DWDie rtn;
            if(TryGetDIE(code, out rtn))
                return rtn;
            else throw new NotSupportedException(string.Format("Operation not supported for attribute: {0}", code));
        }
        #endregion

        #region GetUData
        public bool TryGetUData(DWAttrType code, out UInt64 rtn)
        {
            DWAttribute attr;
            if (TryGetAttribute(code, out attr))
            {
                if(attr.TryGetUData(_cu, _cu._debugInfo, out rtn))
                {
                    // Offset the references relative to the start of the cu to force the same behavior across dwarf versions
                    //rtn = RectifyReference(rtn, attr._formCode);
                    return true;
                }
            }
            rtn = 0;
            return false;
        }
        public UInt64 GetUData(DWAttrType code)
        {
            UInt64 rtn;
            if(TryGetUData(code, out rtn))
                return rtn;
            else throw new NotSupportedException(string.Format("Operation not supported for attribute: {0}", code));
        }
        #endregion

        #region GetSData
        public bool TryGetSData(DWAttrType code, out Int64 rtn)
        {
            DWAttribute attr;
            if (TryGetAttribute(code, out attr))
                return attr.TryGetSData(_cu._debugInfo, out rtn);
            rtn = 0;
            return false;

        }
        public Int64 GetSData(DWAttrType code)
        {
            Int64 rtn;
            if (TryGetSData(code, out rtn))
                return rtn;
            else throw new NotSupportedException(string.Format("Operation not supported for attribute: {0}", code));
        }
        #endregion

        public DWDie()
        {
            _tag = DWTag.DW_TAG_padding;
            _labelCache = null;
            _typeInfo = null;
            _memberOf = UInt32.MaxValue;
            _sibling = UInt32.MaxValue;
            _linkLabelCache = null;
            _externalLink = false;
        }
        public DWDie(DWCompilationUnitHeader cu)
        {
            Extract(cu);
        }

        public void Extract(DWCompilationUnitHeader cu)
        {
            _cu = cu;
            _tag = DWTag.DW_TAG_padding;
            _labelCache = null;
            _typeInfo = null;
            _memberOf = UInt32.MaxValue;
            _sibling = UInt32.MaxValue;
            _linkLabelCache = null;
            _externalLink = false;

            if (_cu._context._dwarfVersion > 1)
                CreateDIEver2_4();
            else
                CreateDIEver1();
        }

        private void CreateDIEver1()
        {
            MemoryUnit info = _cu._debugInfo;
            _offset = (UInt32)info.CurrentAddress;
            _length = info.GetUInt32();
            //_childEntries = new List<DwarfDebuggingInformationEntry>();

            if (_length < 8) //NULL entry skip remaining bytes (should be all 0's)
            {
                _tag = 0;
                //byte[] zeroes = reader.ReadBytes((int)_length-4);
                //Console.WriteLine(" {0,20} {1:X4}", _tag, _length);
            }
            else
            {
                _tag = (DWTag)info.GetUInt16();

                UInt32 end = _offset + _length;
                Int32 count = 0;
                while (info.CurrentAddress < end)
                {
                    _cu._context._attributeCache[count] = new DWAttribute(info);
                    _cu._context._attributeCache[count].InitializeAndSkip(this, info, null); // no strtable in DWARF 1.1
                    count++;
                }

                if (_sibling == UInt32.MaxValue || _sibling < _offset + _length)
                    _sibling = _offset + _length;

                _attributeCount = (byte)count;
                if(_attributes == null || _attributes.Length < count)
                    _attributes = new DWAttribute[count];
                if(count > 0)
                    Array.Copy(_cu._context._attributeCache, _attributes, count);
            }
        }

        private void CreateDIEver2_4()
        {
            MemoryUnit info = _cu._debugInfo;
            MemoryUnit strTable = _cu._context._debugStr;
            DWAbbreviationTable abbrev = _cu._context._debugAbbrev;

            _offset = (UInt32)info.CurrentAddress;
            UInt64 abbrevCode = info.GetULEB128();

            //Debug.WriteLine(" Abbrev:{0:X}<{1:X}>", _cu._abbrevOffset, abbrevCode);
            if(abbrevCode != 0)
            {
                abbrev.PopulateDIE(this, _cu._abbrevOffset, (UInt32)abbrevCode);
                //Debug.WriteLine(" <{0:X}><{1:X}> {2,20} {3:X4}", _offset, _offset - _cu._start, _tag, _length);

                //if (_offset == 0x1C4)
                //    Debug.AutoFlush = true;

                for(int ii = 0; ii < _attributeCount; ii++)
                {
                    _attributes[ii].InitializeAndSkip(this, info, strTable);
                    //Debug.WriteLine(GetAttributeString(ref _attributes[ii]));
                }
            }
            else
            {
                //Debug.WriteLine("<NULL>\n");
            }

            _length = (UInt32)(info.CurrentAddress - _offset);
            if (_sibling == UInt32.MaxValue || _sibling < _offset + _length)
                _sibling = _offset + _length;
        }

        public static DWTag GetTag(DWCompilationUnitHeader cu, UInt32 offset)
        {
            if (cu._context._dwarfVersion > 1)
                return GetTagVer2_4(cu, offset);
            else
                return GetTagVer1(cu, offset);
        }

        private static DWTag GetTagVer1(DWCompilationUnitHeader cu, UInt32 offset)
        {
            UInt64 len = cu._debugInfo.GetUInt32((UInt64)offset);
            if (len > 8)
                return (DWTag)cu._debugInfo.GetUInt16();
            else
                return DWTag.DW_TAG_padding;
        }

        private static DWTag GetTagVer2_4(DWCompilationUnitHeader cu, UInt32 offset)
        {
            UInt64 abbrevCode = cu._debugInfo.GetULEB128((UInt64)offset);
            if (abbrevCode != 0)
                return cu._context._debugAbbrev.GetTag(cu._abbrevOffset, (UInt32)abbrevCode);
            else
                return DWTag.DW_TAG_padding;
        }

        public override string ToString()
        {
            var build = ObjectFactory.StringBuilders.GetObject();
            build.Clear();

            build.AppendLine(string.Format("<{0:X}><{1:X}> {2}", _offset, _offset - _cu._start, Tag));
            if (_attributes != null)
            {
                long index = _cu._debugInfo.CurrentIndex;
                for (int ii = 0; ii< _attributeCount; ii++)
                {
                    build.AppendLine(GetAttributeString(ref _attributes[ii]));
                }
                _cu._debugInfo.CurrentIndex = index;
            }
            string rtn = build.ToString();
            ObjectFactory.StringBuilders.ReleaseObject(build);
            return rtn;
        }

        private string GetAttributeString(ref DWAttribute attribute)
        {
            long index = _cu._debugInfo.CurrentIndex;

            string value = attribute.ToString() + " ";

            //Append attribute value
            UInt64 udata;
            Int64 sdata;
            byte flag;
            DWExpression expr;
            DWTypeInfo type;
            string str;
            int syncId;
            MemoryUnit block;
            if (attribute.TryGetString(_cu._debugInfo, _cu._context._debugStr, out str))
                value += str;
            else if (attribute.TryGetTypeInfo(_cu, _cu._debugInfo, out type))
                value += type.ToString();
            else if (attribute.TryGetUData(_cu, _cu._debugInfo, out udata))
                value += udata.ToString("X");
            else if (attribute.TryGetSData(_cu._debugInfo, out sdata))
                value += sdata < 0 ? ("-" + (-sdata).ToString("X")) : sdata.ToString("X");
            else if (attribute.TryGetFlag(_cu._debugInfo, out flag))
                value += flag;
            else if (attribute.TryGetExpression(_cu._debugInfo, _cu._context, out expr))
                value += expr.ToString();
            else if (TryGetBlock(attribute._typeCode, out block, out syncId))
            {
                foreach (byte val in block.GetBytes())
                    value += Utilities.Byte2HexTable[val];

                block.DesyncCacheRange(syncId);
            }

            _cu._debugInfo.CurrentIndex = index;

            return value;
        }
    }

}
