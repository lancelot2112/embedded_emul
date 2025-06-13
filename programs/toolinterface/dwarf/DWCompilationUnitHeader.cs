using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Types;
using EmbedEmul.Binary;
using EmbedEmul.Variables;
using EmbedEmul.Elf;
using EmbedEmul.Memory;

namespace EmbedEmul.Programs.TIS.Dwarf
{
    public class DWCompilationUnitHeader
    {
        internal Int32 _index;
        internal UInt32 _start;
        internal UInt32 _dieStart;
        internal UInt32 _end;
        internal byte _size;
        internal UInt32 _length;
        internal UInt16 _version;
        internal UInt32 _abbrevOffset;
        internal byte _addressSize;
        internal UInt32 _dieCount;

        internal Dictionary<UInt32, GenType> _typeCache;
        //internal Dictionary<UInt32, DWDie> _dieCache;
        internal Dictionary<string, UInt32> _labelCache;
        internal Dictionary<string, UInt32> _linkLabelCache;
        internal Dictionary<UInt32, List<UInt32>> _indirectMemberCache;
        //internal List<UInt32> _dieOffsets;
        //internal Dictionary<string, List<UInt32>> _offsetsByName;
        internal MemoryUnit _debugInfo;
        internal MemoryUnit _debugLine;
        internal DWDebug _context;
        internal DWDie _cuDIE;

        public DWDie DIE { get { return _cuDIE; } }

        public DWCompilationUnitHeader(Int32 index, DWDebug context, MemoryUnit debugInfo, MemoryUnit lineInfo)
        {
            _index = index;
            _context = context;
            _debugInfo = debugInfo;
            _debugLine = lineInfo;
            _start = (UInt32)debugInfo.CurrentAddress;

            if (_context._dwarfVersion > 1)
            {
                _length = debugInfo.GetUInt32();
                if (_length <= 0xfffffff0) _size = 11;
                else throw new NotSupportedException("64bit DWARF not supported, CompilationUnitHeader.");
                _version = debugInfo.GetUInt16();
                _abbrevOffset = debugInfo.GetUInt32();
                _addressSize = debugInfo.GetUInt8();

                _dieStart = (UInt32)_debugInfo.CurrentAddress;
                _cuDIE = new DWDie(this);
                if (_length > 0)
                    _end = _start + 4 + _length;
                else
                    _end = _start;
            }
            else
            {
                _dieStart = (UInt32)_debugInfo.CurrentAddress;
                _cuDIE = new DWDie(this);
                _version = 1;
                _abbrevOffset = 0;
                _addressSize = 4; //Currently only support size of 4 bytes
                _size = 0; //no formal cu
                Int64 curridx = _debugInfo.CurrentIndex;
                _length = (UInt32)(_cuDIE.GetUData(DWAttrType.DW_AT_sibling) - _start);
                _debugInfo.CurrentIndex = curridx;
                _end = _start + _length;
            }
            //Debug.Assert(_cuDIE.Tag == DWTag.DW_TAG_compile_unit,"Not a CompilationUnit","Start at {0:X}.",_dieStart,_end);
        }

        public void ClearCache()
        {
            if(_typeCache!=null)
                _typeCache.Clear();
        }

        public void PopulateHashTables()
        {
            int initialSize = (int)(_length / 150);
            _typeCache = new Dictionary<UInt32, GenType>(initialSize);
            _labelCache = new Dictionary<string, UInt32>(initialSize);
            _linkLabelCache = new Dictionary<string, UInt32>(5);
            _indirectMemberCache = new Dictionary<UInt32, List<UInt32>>(5);
            _dieCount = 0;
            GenType type;
            foreach(DWDie die in GetDIEs())
            {
                _dieCount++;
                if (die.Tag != DWTag.DW_TAG_padding)
                {
                    if (die._tag == DWTag.DW_TAG_global_variable ||
                        (die._tag == DWTag.DW_TAG_variable) ||
                        die._tag == DWTag.DW_TAG_global_subroutine ||
                        (die._tag == DWTag.DW_TAG_subprogram && !die._externalLink) ||
                        die._tag == DWTag.DW_TAG_subroutine ||
                        die._tag == DWTag.DW_TAG_typedef)
                    {
                        if (die._labelCache != null && !_labelCache.ContainsKey(die._labelCache))
                        {
                            _labelCache.Add(die._labelCache, die._offset);
                            _context.RegisterName(die._labelCache, this);
                        }
                        if (die._linkLabelCache != null && !_linkLabelCache.ContainsKey(die._linkLabelCache))
                        {
                            _linkLabelCache.Add(die._linkLabelCache, die._offset);
                            _context.RegisterName(die._linkLabelCache, this);
                        }
                        if (die._memberOf < UInt32.MaxValue)
                        {
                            List<UInt32> members;
                            if (_indirectMemberCache.TryGetValue((UInt32)die._memberOf, out members))
                                members.Add(die._offset);
                            else
                                _indirectMemberCache.Add((UInt32)die._memberOf, new List<UInt32>() { die._offset });
                        }
                        //Helps indirectly set var types since mapping back to symbol names is easier than symbol to debug names apparently
                        if (die._tag != DWTag.DW_TAG_typedef)
                            TryGetType(die, 0, out type);
                    }
                }
                die.Release();
            }
        }

        public IEnumerable<DWDie> GetDIEs()
        {
            if (_length == 0)
                yield break;

            UInt32 offset = _dieStart;
            while (offset < _end)
            {
                DWDie next = GetDIE(offset);
                offset += next._length;
                yield return next;
            }
        }

        public DWDie GetDIE(UInt32 offset)
        {
            DWDie rtnDie;
            _debugInfo.CurrentAddress = offset;
            rtnDie = DWDie.Cache.GetObject();
            if (offset >= _start && offset < _end)
            {
               rtnDie.Extract(this);
            }
            else
            {
                DWCompilationUnitHeader compUnit = _context._cus.Where(cu => offset >= cu._start && offset < cu._end).First();
                rtnDie.Extract(compUnit);
            }

            //if (offset == 0x43d)
            //    Debug.AutoFlush = true;
            return rtnDie;
        }
        public IEnumerable<DWDie> GetDIEsByLabel(string label, HashSet<DWTag> tags, Func<string, string, bool> labelPredicate = null)
        {
            if (_length == 0)
                yield break;

            if (labelPredicate == null)
                labelPredicate = ContainsPredicate;

            DWDie die;
            UInt32 cacheOffset;
            if (labelPredicate != ContainsPredicate)
            {
                if(_labelCache.TryGetValue(label, out cacheOffset) || _linkLabelCache.TryGetValue(label, out cacheOffset))
                    if (CheckTagAtOffset(cacheOffset, tags, out die))
                        yield return die;
            }
            else
            {
                foreach(KeyValuePair<string, UInt32> labelOffset in _labelCache)
                {
                    if(labelPredicate(label, labelOffset.Key) && CheckTagAtOffset(labelOffset.Value, tags, out die))
                        yield return die;
                }
                foreach(KeyValuePair<string, UInt32> linkOffset in _linkLabelCache)
                {
                    if (labelPredicate(label, linkOffset.Key) && CheckTagAtOffset(linkOffset.Value, tags, out die))
                        yield return die;
                }
            }
        }

        private bool CheckTagAtOffset(UInt32 offset, HashSet<DWTag> tags, out DWDie die)
        {
            die = null;
            if (_length == 0)
                return false;

            if (tags != null)
            {
                DWTag rtnTag = DWDie.GetTag(this, offset);
                if (tags.Contains(rtnTag))
                    die = GetDIE(offset);
                else die = null;
            }
            else die = GetDIE(offset);

            return die != null;
        }

        private static HashSet<DWTag> _GlobalVariableTags_ = new HashSet<DWTag>()
        {
            DWTag.DW_TAG_global_variable,
            DWTag.DW_TAG_subroutine,
            DWTag.DW_TAG_subprogram,
            DWTag.DW_TAG_global_subroutine,
            DWTag.DW_TAG_variable
        };
        public bool TryGetGlobalVariableDIE(string name, out DWDie die)
        {
            die = null;
            if (_length == 0)
                return false;

            die = GetDIEsByLabel(name, _GlobalVariableTags_, EqualsPredicate).FirstOrDefault();

            if(die.Tag == DWTag.DW_TAG_variable)
            {
                if (die.GetFlag(DWAttrType.DW_AT_external) != 1)
                    die = null;
            }

            return die != null;
        }

        private static HashSet<DWTag> _TypeDefTags_ = new HashSet<DWTag>()
        {
            DWTag.DW_TAG_typedef
        };
        public bool TryGetTypeDefDIE(string name, out DWDie die)
        {
            die = null;
            if (_length == 0)
                return false;

            die = GetDIEsByLabel(name, _TypeDefTags_, EqualsPredicate).FirstOrDefault();
            return die != null;
        }

        public bool ContainsPredicate(string s1, string s2)
        {
            return s1.Contains(s2);
        }
        public bool EqualsPredicate(string s1, string s2)
        {
            return s1.Equals(s2);
        }

        public bool TryGetType(string name, UInt64 size, out GenType type)
        {
            type = null;
            _typeCache.Clear();

            if (_length == 0)
                return false;

            DWDie entry;
            if (TryGetGlobalVariableDIE(name, out entry))
            {
                TryGetType(entry, size, out type);
                entry.Release();
            }
            else if(TryGetTypeDefDIE(name, out entry))
            {
                TryGetType(entry, size, out type);
                entry.Release();
            }

            if (type != null && size != 0 && (ulong)type.ByteSize != size)
                type.ByteSize = (long)size;

            return type != null;
        }

        public bool TryGetType(DWDie entry, UInt64 size, out GenType type)
        {
            type = null;
            _typeCache.Clear();

            if (_length == 0)
                return false;
            if (_context._dwarfVersion > 1)
                type = _getAssemblyVer2_4(entry, size, entry._offset);
            else
                type = _getAssembly(entry, size);
            return type != null;
        }

        public DWSourceStatement GetSourceStatement()
        {
            ulong rtn;
            if (_context._debugLine != null && _length != 0 && _cuDIE.TryGetUData(DWAttrType.DW_AT_stmt_list, out rtn))
                return new DWSourceStatement(_debugLine, (uint)rtn);
            else
                return null;
        }

        private GenType _getAssemblyVer2_4(DWDie entry, UInt64 size, UInt32 saveOffset, string name = null)
        {
            GenType assembly;
            if (entry == null)
                return null;

            if (!_typeCache.TryGetValue(saveOffset, out assembly))
            {
                //TODO: on cases not handled exit gracefully and don't return an assembly (ie. fail on this one and move on so we can get some information)
                if (!entry.TryGetString(DWAttrType.DW_AT_name, out name))
                    name = "";
                switch (entry.Tag)
                {
                    case DWTag.DW_TAG_typedef:
                        {
                            DWDie typeDie = entry.GetDIE(DWAttrType.DW_AT_type);
                            assembly = _getAssemblyVer2_4(typeDie, size, typeDie._offset);
                            assembly._name = name;
                            if(!_typeCache.ContainsKey(saveOffset))
                              _typeCache.Add(saveOffset, assembly);
                            break;
                        }
                    case DWTag.DW_TAG_base_type:
                        {
                            assembly = null;
                            uint byteSize = (uint)entry.GetUData(DWAttrType.DW_AT_byte_size);
                            Debug.Assert(byteSize != 0);

                            var baseType = (DWBaseType)entry.GetUData(DWAttrType.DW_AT_encoding);
                            switch (baseType)
                            {
                                //case DWBaseType.DW_ATE_address: break;
                                case DWBaseType.DW_ATE_boolean: assembly = new GenBaseValue(name, byteSize, ValueEncoding.Unsigned, DisplayFormat.hex); break;
                                //case DWBaseType.DW_ATE_complex_float: break;
                                //case DWBaseType.DW_ATE_decimal_float: break;
                                //case DWBaseType.DW_ATE_edited: break;
                                case DWBaseType.DW_ATE_float: assembly = new GenBaseValue(name, byteSize, ValueEncoding.Floating); break;
                                //case DWBaseType.DW_ATE_imaginary_float: break;
                                //case DWBaseType.DW_ATE_numeric_string: break;
                                //case DWBaseType.DW_ATE_packed_decimal: break;
                                case DWBaseType.DW_ATE_signed: assembly = new GenBaseValue(name, byteSize, ValueEncoding.Signed); break;
                                case DWBaseType.DW_ATE_signed_char: assembly = new GenBaseValue(name, byteSize , ValueEncoding.Signed); break;
                                //case DWBaseType.DW_ATE_signed_fixed: break;
                                case DWBaseType.DW_ATE_unsigned: assembly = new GenBaseValue(name, byteSize, ValueEncoding.Unsigned, DisplayFormat.hex); break;
                                case DWBaseType.DW_ATE_unsigned_char: assembly = new GenBaseValue(name, byteSize, ValueEncoding.Unsigned, DisplayFormat.hex); break;
                                //case DWBaseType.DW_ATE_unsigned_fixed: break;
                                //case DWBaseType.DW_ATE_UTF: break;
                                default: throw new NotImplementedException("Type " + baseType + " not implemented.");
                            }

                            if (assembly == null) throw new NotImplementedException();

                            _typeCache.Add(saveOffset, assembly);
                            break;
                        }
                    case DWTag.DW_TAG_union_type:
                        {

                            GenStructure union = new GenStructure(name);
                            UInt64 byteSize;
                            if (entry.TryGetUData(DWAttrType.DW_AT_byte_size, out byteSize))
                                union._byteSize = (Int64)byteSize;
                            else
                                union._byteSize = -1;
                            assembly = union;
                            _typeCache.Add(saveOffset, assembly);

                            foreach (DWDie subentry in entry.Children())
                            {
                                if (subentry.Tag == DWTag.DW_TAG_member)
                                {
                                    //Get member name
                                    string membername = subentry.GetString(DWAttrType.DW_AT_name);
                                    //Get member type
                                    GenType sub_assembly = _getAssemblyVer2_4(subentry.GetDIE(DWAttrType.DW_AT_type), 0, subentry._offset);
                                    union.AddMember(sub_assembly, membername, 0);
                                }
                                subentry.Release();
                            }
                            union.Finish();
                            break;
                        }
                    case DWTag.DW_TAG_structure_type:
                        {
                            GenStructure structure = new GenStructure(name);
                            UInt64 byteSize;
                            if (entry.TryGetUData(DWAttrType.DW_AT_byte_size, out byteSize))
                                structure._byteSize = (Int64)byteSize;
                            else
                                structure._byteSize = -1;

                            assembly = structure;
                            _typeCache.Add(saveOffset, assembly);

                            foreach (DWDie subentry in entry.Children())
                            {
                                if (subentry.Tag == DWTag.DW_TAG_member)
                                {
                                    //Get member name
                                    string membername = subentry.GetString(DWAttrType.DW_AT_name);
                                    //Get member type
                                    GenType sub_assembly = _getAssemblyVer2_4(subentry.GetDIE(DWAttrType.DW_AT_type), 0, subentry._offset);
                                    //Get member byte location
                                    DWExpression expression = subentry.GetExpression(DWAttrType.DW_AT_data_member_location);
                                    UInt32 mem_loc = (UInt32)expression.Operate(0);
                                    //Get bit offsets
                                    UInt64 bitSize;
                                    UInt64 bitOffset;
                                    if (subentry.TryGetUData(DWAttrType.DW_AT_bit_size, out bitSize))
                                    {
                                        if (!subentry.TryGetUData(DWAttrType.DW_AT_bit_offset, out bitOffset))
                                            bitOffset = UInt64.MaxValue;
                                    }
                                    else
                                    {
                                        bitSize = UInt64.MaxValue;
                                        bitOffset = UInt64.MaxValue;
                                    }

                                    structure.AddMember(sub_assembly, membername, mem_loc, (int)((Int64)bitOffset), (int)((Int64)bitSize));
                                }
                                subentry.Release();
                            }

                            structure.Finish();
                            //Debug.Assert(structure._calcByteSize == structure._byteSize);

                            break;
                        }
                    case DWTag.DW_TAG_enumeration_type:
                        {
                            uint byteSize = (uint)entry.GetUData(DWAttrType.DW_AT_byte_size);
                            GenEnumeration enumeration = new GenEnumeration(name, (int)byteSize, new List<ElementListEntry>());

                            foreach (DWDie subentry in entry.Children())
                            {
                                if (subentry.Tag == DWTag.DW_TAG_enumerator)
                                {
                                    enumeration.AddEnumeration(subentry.GetString(DWAttrType.DW_AT_name),
                                                               subentry.GetSData(DWAttrType.DW_AT_const_value));
                                }
                                subentry.Release();
                            }

                            assembly = enumeration;
                            _typeCache.Add(saveOffset, assembly);
                            break;
                        }
                    case DWTag.DW_TAG_pointer_type:
                        {
                            UInt64 byteSize;
                            if (!entry.TryGetUData(DWAttrType.DW_AT_byte_size, out byteSize))
                                byteSize = _addressSize;
                            GenPointer pointer = new GenPointer((GenType)null, (Int64)byteSize);
                            assembly = pointer;
                            _typeCache.Add(saveOffset, assembly);
                            DWDie pointerTypeDie;
                            if (entry.TryGetDIE(DWAttrType.DW_AT_type, out pointerTypeDie))
                            {
                                pointer.SetType(_getAssemblyVer2_4(pointerTypeDie, 0, pointerTypeDie._offset));
                                pointerTypeDie.Release();
                            }
                            else pointer.SetType(null);
                            assembly._byteSize = (long)byteSize;
                            break;
                        }
                    case DWTag.DW_TAG_string_type:
                        {
                            throw new NotImplementedException();
                            //break;
                        }
                    case DWTag.DW_TAG_array_type:
                        {
                            DWDie typeDie = entry.GetDIE(DWAttrType.DW_AT_type);
                            GenType type = _getAssemblyVer2_4(typeDie, 0, typeDie._offset);
                            //Subrange type to get index information
                            DWDie subRangeTypeDIE;
                            GenType subRangeType;
                            UInt64 upper = 0, lower = 0;
                            foreach (DWDie child in entry.Children())
                            {
                                if (child.Tag == DWTag.DW_TAG_subrange_type)
                                {
                                    if (child._attributes.Length > 0)
                                    {
                                        if (child.TryGetDIE(DWAttrType.DW_AT_type, out subRangeTypeDIE))
                                            subRangeType = _getAssemblyVer2_4(subRangeTypeDIE, 0, child._offset);
                                        if (!child.TryGetUData(DWAttrType.DW_AT_lower_bound, out lower)) lower = 0;
                                        if (!child.TryGetUData(DWAttrType.DW_AT_upper_bound, out upper)) upper = 0;
                                    }
                                    child.Release();
                                    break;
                                }
                                else child.Release();
                            }
                            assembly = new GenArray(name, type, (Int64)lower, (Int64)upper);

                            if ((long)size > assembly.ByteSize)
                                assembly.SetByteSize((long)size);
                            _typeCache.Add(saveOffset, assembly);
                            break;
                        }
                    case DWTag.DW_TAG_volatile_type:
                        {
                            //TODO: Add modifier?
                            DWDie volTypeDIE;
                            if (entry.TryGetDIE(DWAttrType.DW_AT_type, out volTypeDIE))
                            {
                                GenType type = _getAssemblyVer2_4(volTypeDIE, size, volTypeDIE._offset);
                                assembly = type;
                                _typeCache.Add(saveOffset, assembly);
                                volTypeDIE.Release();
                            }
                            break;
                        }
                    case DWTag.DW_TAG_const_type:
                        {
                            //TODO: Add modifier?
                            DWDie volTypeDIE;
                            if (entry.TryGetDIE(DWAttrType.DW_AT_type, out volTypeDIE))
                            {
                                GenType type = _getAssemblyVer2_4(volTypeDIE, size, volTypeDIE._offset);
                                assembly = type;
                                if(!_typeCache.ContainsKey(saveOffset))
                                    _typeCache.Add(saveOffset, assembly);
                                volTypeDIE.Release();
                            }
                            break;
                        }
                    case DWTag.DW_TAG_subroutine_type:
                        assembly = _getSubroutine(entry, name, saveOffset);
                        break;
                    case DWTag.DW_TAG_subprogram:
                        assembly = _getSubroutine(entry, name, saveOffset);
                        break;
                    case DWTag.DW_TAG_subroutine:
                        assembly = _getSubroutine(entry, name, saveOffset);
                        break;
                    case DWTag.DW_TAG_global_subroutine:
                        assembly = _getSubroutine(entry, name, saveOffset);
                        break;
                    case DWTag.DW_TAG_class_type:
                        {
                            UInt32 byteSize = (UInt32)entry.GetUData(DWAttrType.DW_AT_byte_size);
                            GenClass classAssy = new GenClass(name, byteSize);
                            assembly = classAssy;
                            _typeCache.Add(saveOffset, assembly);

                            //foreach (DWDie subentry in entry.Children())
                            //{

                            //}

                            break;
                        }
                    case DWTag.DW_TAG_unspecified_type:
                        {
                           GenBaseValue baseValue = new GenBaseValue(name, 0, ValueEncoding.Unsigned, DisplayFormat.hex);
                           assembly = baseValue;
                           _typeCache.Add(saveOffset, assembly);
                           break;
                        }
                    default:
                        DWDie typeDIE;
                        if (entry.TryGetDIE(DWAttrType.DW_AT_type, out typeDIE)) //some kind of variable or some other thing... try to get to a typedef
                        {
                            assembly = _getAssemblyVer2_4(typeDIE, size, typeDIE._offset);
                            typeDIE.Release();
                        } //else there was no type to be had
                        break;//throw new NotImplementedException(string.Format("{0} not implemented.", entry.Tag));

                }
            }
            return assembly;
        }

        private GenSubroutine _getSubroutine(DWDie entry, string name, UInt32 saveOffset)
        {
            GenSubroutine subroutine = new GenSubroutine(name);
            _typeCache.Add(saveOffset, subroutine);

            UInt64 lowPC, highPC;
            if (entry.TryGetUData(DWAttrType.DW_AT_low_pc, out lowPC) &&
               entry.TryGetUData(DWAttrType.DW_AT_high_pc, out highPC))
                subroutine.SetPC(lowPC, highPC);
            else subroutine._isDynamicSize = true; //ie. it's a function prototype only not a full static definition

            DWDie retType;
            if (entry.TryGetDIE(DWAttrType.DW_AT_type, out retType))
            {
                subroutine._returnTypes.Add(_getAssemblyVer2_4(retType, 0, retType._offset));
                retType.Release();
            }
            else subroutine._returnTypes.Add(null);

            foreach (DWDie subentry in entry.Children())
            {
                if (subentry.Tag == DWTag.DW_TAG_formal_parameter)
                {
                    DWDie abstractEntry = subentry;
                    UInt32 subentryOffset = subentry._offset;
                    DWDie newEntry;
                    while (abstractEntry.TryGetDIE(DWAttrType.DW_AT_abstract_origin, out newEntry))
                    {
                        if (newEntry != null)
                        {
                            //subentry might become invalid here
                            abstractEntry.Release();
                            abstractEntry = newEntry;
                        }
                        else break;
                    };

                    DWDie subentryTypeDie = abstractEntry.GetDIE(DWAttrType.DW_AT_type);
                    GenType inputType = _getAssemblyVer2_4(subentryTypeDie, 0, subentryOffset);
                    subroutine._inputTypes.Add(inputType);
                    subentryTypeDie.Release();

                    abstractEntry.Release();
                }
                else if (subentry.Tag == DWTag.DW_TAG_variable || subentry.Tag == DWTag.DW_TAG_local_variable)
                {
                    //GenType localVariable = _getAssemblyVer2_4(subentry.GetDIE(DWAttrType.DW_AT_type), 0);
                    //subroutine._localVars.Add(localVariable);
                    subentry.Release();
                }
                else subentry.Release();
            }
            return subroutine;
        }

        private DWTypeInfo _getType(DWDie entry, UInt64 size)
        {
            //Check to see if this is a fundamental type or not
            DWTypeInfo typeInfo;
            if (_context._dwarfVersion == 1)
            {
                if (entry.TryGetTypeInfo(DWAttrType.DW_AT_fund_type, out typeInfo)) ;
                else if (entry.TryGetTypeInfo(DWAttrType.DW_AT_mod_fund_type, out typeInfo)) ;
                else if (entry.TryGetTypeInfo(DWAttrType.DW_AT_user_def_type, out typeInfo)) ;
                else if (entry.TryGetTypeInfo(DWAttrType.DW_AT_mod_u_d_type, out typeInfo)) ;
                else throw new NotSupportedException();
            }
            else
            {
                if (entry.TryGetTypeInfo(DWAttrType.DW_AT_base_types, out typeInfo)) ;
                else if (entry.TryGetTypeInfo(DWAttrType.DW_AT_type, out typeInfo)) ;
                else throw new NotSupportedException();
            }

            return typeInfo;

        }
        private GenType _getAssembly(DWDie entry,  UInt64 size, DWTypeInfo typeInfo = null, string parentTypeName = null, string parentVarName = null)
        {
            GenType returnAssembly = null;
            if (typeInfo == null || typeInfo._attr.TypeCode == DWAttrType.DW_AT_user_def_type || typeInfo._attr.TypeCode == DWAttrType.DW_AT_mod_u_d_type)
                returnAssembly = _getUserDefinedType(entry, typeInfo, parentTypeName, parentVarName, size);
            else if (typeInfo._attr.TypeCode == DWAttrType.DW_AT_fund_type || typeInfo._attr.TypeCode == DWAttrType.DW_AT_mod_fund_type)
                returnAssembly = _getFundamentalType(entry, typeInfo, parentTypeName);
            else
                throw new NotSupportedException();

            return returnAssembly;
        }

        private static Dictionary<DWFundamentalType, string> _fundamentalTypeNames = new Dictionary<DWFundamentalType, string>()
        {
            {DWFundamentalType.DW_FT_char, "char"},
            {DWFundamentalType.DW_FT_unsigned_char, "unsigned char"},
            {DWFundamentalType.DW_FT_signed_char, "char"},
            {DWFundamentalType.DW_FT_short, "short"},
            {DWFundamentalType.DW_FT_unsigned_short, "unsigned short"},
            {DWFundamentalType.DW_FT_signed_short, "short"},
            {DWFundamentalType.DW_FT_integer, "int"},
            {DWFundamentalType.DW_FT_unsigned_integer, "unsigned int"},
            {DWFundamentalType.DW_FT_signed_integer, "int"},
            {DWFundamentalType.DW_FT_long, "long"},
            {DWFundamentalType.DW_FT_unsigned_long, "unsigned long"},
            {DWFundamentalType.DW_FT_signed_long, "long"},
            {DWFundamentalType.DW_FT_unsigned_long_long, "unsinged long long"},
            {DWFundamentalType.DW_FT_signed_long_long, "long long"},
            {DWFundamentalType.DW_FT_float, "float"},
            {DWFundamentalType.DW_FT_dbl_prec_float, "double"},
            {DWFundamentalType.DW_FT_void, "void"},
            {DWFundamentalType.DW_FT_boolean, "boolean" }
        };

        private static Dictionary<DWFundamentalType, int> _funTypeSizes = new Dictionary<DWFundamentalType, int>()
        {
            {DWFundamentalType.DW_FT_char, 1},
            {DWFundamentalType.DW_FT_unsigned_char, 1},
            {DWFundamentalType.DW_FT_signed_char, 1},
            {DWFundamentalType.DW_FT_short, 2},
            {DWFundamentalType.DW_FT_unsigned_short, 2},
            {DWFundamentalType.DW_FT_signed_short, 2},
            {DWFundamentalType.DW_FT_integer, 4},
            {DWFundamentalType.DW_FT_unsigned_integer, 4},
            {DWFundamentalType.DW_FT_signed_integer, 4},
            {DWFundamentalType.DW_FT_long, 4},
            {DWFundamentalType.DW_FT_unsigned_long, 4},
            {DWFundamentalType.DW_FT_signed_long, 4},
            {DWFundamentalType.DW_FT_unsigned_long_long, 8},
            {DWFundamentalType.DW_FT_signed_long_long, 8},
            {DWFundamentalType.DW_FT_float, 4},
            {DWFundamentalType.DW_FT_dbl_prec_float, 8},
            {DWFundamentalType.DW_FT_void, 0},
            {DWFundamentalType.DW_FT_boolean, 1}
        };

        private GenType _getFundamentalType(DWDie parent, DWTypeInfo typeInfo, string typeName)
        {
            GenType returnAssembly;
            DWFundamentalType funType = (DWFundamentalType)typeInfo._value;
            //if (!_fundamentalTypes.TryGetValue(funType.Type, out returnAssembly))
            //    throw new NotImplementedException(string.Format("{0} not supported.", funType.Type));

            if (string.IsNullOrEmpty(typeName))
                typeName = _fundamentalTypeNames[funType];

            //if (!_fundamentalTypes.TryGetValue(funType.Type, out returnAssembly))
            //    throw new NotImplementedException(funType.Type.ToString() + " not implemented.");

            switch (funType)
            {
                case DWFundamentalType.DW_FT_char: returnAssembly = new GenBaseValue(typeName, 1, ValueEncoding.Signed); break;
                case DWFundamentalType.DW_FT_unsigned_char: returnAssembly = new GenBaseValue(typeName, 1, ValueEncoding.Unsigned, DisplayFormat.hex); break;
                case DWFundamentalType.DW_FT_signed_char: returnAssembly = new GenBaseValue(typeName, 1, ValueEncoding.Signed); break;
                case DWFundamentalType.DW_FT_short: returnAssembly = new GenBaseValue(typeName, 2, ValueEncoding.Signed); break;
                case DWFundamentalType.DW_FT_unsigned_short: returnAssembly = new GenBaseValue(typeName, 2, ValueEncoding.Unsigned, DisplayFormat.hex); break;
                case DWFundamentalType.DW_FT_signed_short: returnAssembly = new GenBaseValue(typeName, 2, ValueEncoding.Signed); break;
                case DWFundamentalType.DW_FT_integer: returnAssembly = new GenBaseValue(typeName, 4, ValueEncoding.Signed); break;
                case DWFundamentalType.DW_FT_unsigned_integer: returnAssembly = new GenBaseValue(typeName, 4, ValueEncoding.Unsigned, DisplayFormat.hex); break;
                case DWFundamentalType.DW_FT_signed_integer: returnAssembly = new GenBaseValue(typeName, 4, ValueEncoding.Signed); break;
                case DWFundamentalType.DW_FT_long: returnAssembly = new GenBaseValue(typeName, 4, ValueEncoding.Signed); break;
                case DWFundamentalType.DW_FT_unsigned_long: returnAssembly = new GenBaseValue(typeName, 4, ValueEncoding.Unsigned, DisplayFormat.hex); break;
                case DWFundamentalType.DW_FT_signed_long: returnAssembly = new GenBaseValue(typeName, 4, ValueEncoding.Signed); break;
                case DWFundamentalType.DW_FT_unsigned_long_long: returnAssembly = new GenBaseValue(typeName, 8, ValueEncoding.Unsigned, DisplayFormat.hex); break;
                case DWFundamentalType.DW_FT_signed_long_long: returnAssembly = new GenBaseValue(typeName, 8, ValueEncoding.Signed); break;
                case DWFundamentalType.DW_FT_float: returnAssembly = new GenBaseValue(typeName, 4, ValueEncoding.Floating); break;
                case DWFundamentalType.DW_FT_dbl_prec_float: returnAssembly = new GenBaseValue(typeName, 8, ValueEncoding.Floating); break;
                case DWFundamentalType.DW_FT_boolean: returnAssembly = new GenBaseValue(typeName, 1, ValueEncoding.Unsigned, DisplayFormat.hex); break;
                case DWFundamentalType.DW_FT_void: returnAssembly = null; break;
                default: throw new NotImplementedException(funType.ToString() + " not implemented.");
            }

            //Check for any type modifications (pointer, const, etc.)
            return FinalizeUserType(parent, typeInfo, returnAssembly);
        }

        private GenType _getUserDefinedType(DWDie parent, DWTypeInfo typeInfo, string typeName, string varName, UInt64 size)
        {
            DWDie entry = typeInfo != null ? GetDIE((UInt32)typeInfo._value) : parent;
            GenType returnAssembly = null;

            if (_typeCache.TryGetValue(entry.Offset, out returnAssembly) && returnAssembly != null)
            {
                if (typeInfo != null)
                    return FinalizeUserType(parent, typeInfo, returnAssembly);
                else return returnAssembly;
            }

            if(entry.Tag == DWTag.DW_TAG_local_variable || entry.Tag == DWTag.DW_TAG_formal_parameter)
            {
                if (string.IsNullOrEmpty(varName))
                    entry.TryGetString(DWAttrType.DW_AT_name, out varName);

                returnAssembly = _getAssembly(entry, size, _getType(entry, size), typeName, varName);
            }
            else if (entry.Tag == DWTag.DW_TAG_global_variable  || entry.Tag == DWTag.DW_TAG_variable)
            {
                if (string.IsNullOrEmpty(varName))
                    entry.TryGetString(DWAttrType.DW_AT_name, out varName);

                returnAssembly = _getAssembly(entry, size, _getType(entry, size), typeName, varName);

                //DWDie parentTypeDIE;
                //GenType parentType = null;
                //if (entry.TryGetDIE(DWAttrType.DW_AT_member, out parentTypeDIE))
                //{
                //    parentType = _getAssembly(parentTypeDIE, 0, _getTypeVer1(parentTypeDIE, 0));
                //}

                //Variable entryVariable;
                //string locationSym;
                //DWExpression expression;
                //if (entry.TryGetString(DWAttrType.DW_AT_data_location, out locationSym))
                //{
                //    _context._symTab.TryGetGlobalSymbol(locationSym, out entryVariable);
                //    if (entryVariable._type == null)
                //        entryVariable.SetType(_getAssembly(entry, size, _getTypeVer1(entry, size), typeName, varName));

                //    returnAssembly = entryVariable._type;
                //}
                //else if (entry.TryGetExpression(DWAttrType.DW_AT_location, out expression))
                //{
                //    if(!_context._symTab.TryGetGlobalSymbol(varName, out entryVariable))
                //    {
                //        entryVariable = new Variable();
                //    }
                //}
                //else entryVariable = null;

                //if (entryVariable != null && parentType != null && parentType is GenClass)
                //{
                //    varName = parent.GetString(DWAttrType.DW_AT_name) + "::" + varName;
                //    entryVariable._label = varName;
                //}
            }
            else
            {
                if (string.IsNullOrEmpty(typeName))
                    entry.TryGetString(DWAttrType.DW_AT_name, out typeName);

                if (entry.Tag == DWTag.DW_TAG_typedef)
                {
                    //TODO: Figure out how to ensure a type is seen and not null

                    if(!_typeCache.ContainsKey(entry.Offset))
                        _typeCache.Add(entry.Offset, null); //Circular dependency resulting in stack overflow needs this catch here

                    returnAssembly = _getAssembly(entry, size, _getType(entry, size), typeName, varName);
                    _typeCache[entry.Offset] = returnAssembly; //Typdef type ends here
                                                               //Now apply any modifiers
                    if(typeInfo != null)
                        returnAssembly = FinalizeUserType(parent, typeInfo, returnAssembly);
                }
                else if (entry.Tag == DWTag.DW_TAG_structure_type)
                {
                    GenStructure structure = new GenStructure(typeName);
                    UInt64 val;
                    if (entry.TryGetUData(DWAttrType.DW_AT_byte_size, out val))
                        structure._byteSize = (UInt32)val;
                    returnAssembly = FinalizeUserType(parent, typeInfo, structure);
                    _typeCache.Add(entry.Offset, returnAssembly);

                    //Loop over children and add any "Member" entries
                    foreach (DWDie child in entry.Children())
                    {
                        if (child.Tag == DWTag.DW_TAG_member)
                        {
                            Int32 bitOffset = -1;
                            Int32 bitSize = -1;
                            UInt64 value = 0;
                            if (child.TryGetUData(DWAttrType.DW_AT_bit_size, out value))
                                bitSize = (Int32)value;

                            if (child.TryGetUData(DWAttrType.DW_AT_bit_offset, out value))
                                bitOffset = (Int32)value;

                            string childName = child.GetString(DWAttrType.DW_AT_name);

                            UInt32 byteOffset;
                            DWExpression locationExpr;
                            if (child.TryGetExpression(DWAttrType.DW_AT_location, out locationExpr))
                            {
                                byteOffset = (UInt32)locationExpr.Operate(0);

                                structure.AddMember(_getAssembly(child, 0, _getType(child, 0)),
                                                    childName,
                                                    byteOffset,
                                                    bitOffset,
                                                    bitSize);
                            }
                            else if (child.LinkLabel != null)
                            { //C++ structures can have global members
                                Variable linkVar;
                                _context._symTab.TryGetGlobalSymbol(child.LinkLabel, out linkVar);
                                linkVar._label = typeName + "::" + childName;

                                if (linkVar._type == null)
                                    linkVar.SetType(_getAssembly(child, 0, _getType(child, 0), childName, varName), _index);

                                _context._symTab.VariableTable.AliasVariable(linkVar._label, child.LinkLabel);
                            }
                            else throw new NotImplementedException("Structure member must have location of some kind...");
                        }
                        child.Release();
                    }

                    structure.Finish();
                }
                else if (entry.Tag == DWTag.DW_TAG_union_type)
                {
                    GenStructure union = new GenStructure(typeName);
                    UInt64 value;
                    if (entry.TryGetUData(DWAttrType.DW_AT_byte_size, out value))
                        union._byteSize = (Int64)value;
                    returnAssembly = FinalizeUserType(parent, typeInfo, union);
                    _typeCache.Add(entry.Offset, returnAssembly);

                    //Loop over children and add any "Member" entries
                    foreach (DWDie child in entry.Children())
                    {
                        if (child.Tag == DWTag.DW_TAG_member)
                        {
                            string name = child.GetString(DWAttrType.DW_AT_name);
                            union.AddMember(_getAssembly(child, 0, _getType(child, 0)), name, 0);
                        }
                        child.Release();
                    }
                    union.Finish();
                }
                else if (entry.Tag == DWTag.DW_TAG_array_type)
                {
                    GenArray array = new GenArray(typeName);
                    returnAssembly = FinalizeUserType(parent, typeInfo, array);
                    _typeCache.Add(entry.Offset, returnAssembly);
                    Int32 syncId;
                    _decodeSubscriptData(parent, entry.GetBlock(DWAttrType.DW_AT_subscr_data, out syncId), syncId, array, size);

                }
                else if (entry.Tag == DWTag.DW_TAG_subroutine_type || entry.Tag == DWTag.DW_TAG_global_subroutine || entry.Tag == DWTag.DW_TAG_subroutine)
                {
                    //Get the return type of the subroutine
                    GenSubroutine subroutine = new GenSubroutine(typeName);
                    if (typeInfo != null)
                        returnAssembly = FinalizeUserType(parent, typeInfo, subroutine);
                    else returnAssembly = subroutine;
                    _typeCache.Add(entry.Offset, returnAssembly);

                    UInt64 lowPC, highPC;
                    if (entry.TryGetUData(DWAttrType.DW_AT_low_pc, out lowPC) &&
                       entry.TryGetUData(DWAttrType.DW_AT_high_pc, out highPC))
                        subroutine.SetPC(lowPC, highPC);
                    else subroutine._isDynamicSize = true; //ie. it's a function prototype only not a full static definition

                    subroutine._returnTypes.Add(_getAssembly(entry, 0, _getType(entry, 0)));

                    foreach (DWDie child in entry.Children())
                    {
                        if (child.Tag == DWTag.DW_TAG_formal_parameter)
                            subroutine._inputTypes.Add(_getAssembly(child, 0));
                        else if (child.Tag == DWTag.DW_TAG_local_variable) //do something
                            ;//subroutine._inputTypes.Add(_getAssembly(child, 0));
                        child.Release();
                    }
                }
                else if (entry.Tag == DWTag.DW_TAG_enumeration_type)
                {
                    UInt32 byteSize = (UInt32)entry.GetUData(DWAttrType.DW_AT_byte_size);
                    GenEnumeration enumeration = new GenEnumeration(typeName, (int)byteSize, entry.GetElementList(DWAttrType.DW_AT_element_list));
                    returnAssembly = FinalizeUserType(parent, typeInfo, enumeration);
                    _typeCache.Add(entry.Offset, returnAssembly);
                }
                else if (entry.Tag == DWTag.DW_TAG_class_type)
                {
                    UInt64 byteSize;
                    if (!entry.TryGetUData(DWAttrType.DW_AT_byte_size, out byteSize))
                        byteSize = 0;

                    GenClass classAssy = new GenClass(typeName, (UInt32)byteSize);
                    returnAssembly = FinalizeUserType(parent, typeInfo, classAssy);
                    _typeCache.Add(entry.Offset, returnAssembly);

                    foreach (DWDie child in entry.Children())
                    {
                        //TODO: Need to get object name for object scope params versus class scope params
                        if (child.Tag == DWTag.DW_TAG_global_subroutine ||
                            child.Tag == DWTag.DW_TAG_global_variable ||
                            child.Tag == DWTag.DW_TAG_subroutine ||
                            child.Tag == DWTag.DW_TAG_member)
                        {
                            string name = null;
                            string locationSym;
                            DWExpression expression;
                            if (child.TryGetString(DWAttrType.DW_AT_data_location, out locationSym))
                            {
                                name = child.GetString(DWAttrType.DW_AT_name);
                                Variable member;
                                _context._symTab.TryGetGlobalSymbol(locationSym, out member);

                                if (member != null) /* If symbol not found then not linked in */
                                {
                                    member._label = typeName + "::" + name;
                                    if (member._type == null)
                                        member.SetType(_getAssembly(child, 0, _getType(child, 0), name, varName), _index);
                                    //Need to be able to lookup variable by name or by id
                                    _context._symTab.VariableTable.AliasVariable(member._label, locationSym);
                                }
                                classAssy.AddStaticMemberLabel(locationSym);
                            }
                            else if (child.TryGetExpression(DWAttrType.DW_AT_location, out expression))
                            {
                                name = child.GetString(DWAttrType.DW_AT_name);
                                var subType = _getAssembly(child, 0, _getType(child, 0), null, varName);
                                ulong offset = expression.Operate(0);
                                classAssy.AddMember(subType, name, (uint)offset);
                            }
                            else throw new NotImplementedException();
                        }
                        child.Release();
                    }
                }
                else
                    throw new NotImplementedException(string.Format("{0} not implemented.", entry.Tag));
            }

            if (entry != parent)
                entry.Release();

            return returnAssembly;
        }

        public GenType FinalizeUserType(DWDie parent, DWTypeInfo typeInfo, GenType unmodifiedAssembly)
        {
            GenType returnAssembly = typeInfo.ApplyModifiers(unmodifiedAssembly);
            return returnAssembly;
        }

        private void _decodeSubscriptData(DWDie parent, MemoryUnit block, Int32 syncId, GenArray array, UInt64 size)
        {
            UInt64 startidx = 0, endidx = 0;

            while (!block.EndOfRange)
            {
                DWArraySubscriptFormat format = (DWArraySubscriptFormat)block.GetUInt8();
                if (format == DWArraySubscriptFormat.DW_FMT_FT_C_C)
                {
                    DWFundamentalType funType = (DWFundamentalType)block.GetUInt16();
                    int typeSize = _funTypeSizes[funType];
                    startidx = block.GetUnsigned(typeSize);
                    endidx = block.GetUnsigned(typeSize);
                }
                //else if (format == DWArraySubscriptFormat.DW_FMT_UT_C_C)
                //{
                //    NumericValue index = (NumericValue)_getAssembly(_dieByOffset[raw.TakeUInt32()]);
                //    index.Build(raw);//start idx
                //    UInt64 startidx = index.UnsignedValue;

                //    index.Build(raw);//end idx
                //    UInt64 endidx = index.UnsignedValue;
                //    arrayAssembly._maxCount = endidx + 1;
                //}
                else if (format == DWArraySubscriptFormat.DW_FMT_ET)
                {
                    DWAttribute attribute = new DWAttribute(block);
                    attribute._location = (UInt64)block.CurrentAddress; //TODO: CurrentAddress?
                    var typeInfo = new DWTypeInfo(this, block, attribute);
                    block.SyncBaseRange();
                    GenType memberType = _getAssembly(null, 0, typeInfo);
                    block.SyncCacheRange(syncId); //need to sync here due to _getAssembly doing who knows what to the stack

                    if (array._maxCount > 0) //check for multidimensional array
                    {
                        GenArray subArray = new GenArray(memberType.Name, memberType, (Int64)startidx, (Int64)endidx);
                        array.SetMemberType(subArray, 0, array._maxCount - 1);
                    }
                    else
                    {
                        if (endidx == 0xFFFFFFFF)
                        {
                            if (size != 0)
                                endidx = (UInt64)((long)size / memberType.ByteSize);
                            else
                                endidx = startidx;
                        }
                        array.SetMemberType(memberType, (Int64)startidx, (Int64)endidx);
                    }
                }
                else
                    throw new NotImplementedException(string.Format("Array subscript decode of {0} not implemented.", format));
            }
            block.DesyncCacheRange(syncId, passIndex: true);
        }

        public override string ToString()
        {
            StringBuilder dieList = new StringBuilder();
            foreach (DWDie die in GetDIEs())
                dieList.Append(die.ToString());
            return dieList.ToString();
        }
    }
}
