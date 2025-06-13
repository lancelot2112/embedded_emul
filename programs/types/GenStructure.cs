using GenericUtilitiesLib;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EmbedEmul.Types
{
    public class GenMember
    {
        internal string _name;
        internal UInt32 _offset;
        internal Int32 _index;
        internal GenType _type;

        public GenMember(string name, UInt32 offset, Int32 index, GenType type)
        {
            _name = name;
            _offset = offset;
            _index = index;
            _type = type;
        }
    }

    /// <summary>
    /// Can contain any number of assembly types in any order (ie. struct.substruct.member)
    /// </summary>
    public class GenStructure : GenType, IGenDynamicSize
    {
        internal List<GenMember> _memberList;
        internal Dictionary<string, GenMember> _members;
        public bool IsDynamicSize
        {
            get { return _hasDynamicSizeMember; }
            set { _hasDynamicSizeMember = value; }
        }
        internal bool _hasDynamicSizeMember;

        public override Int64 ValueCount
        {
            get
            {
                Int64 val = 0;
                if (!_hasDynamicSizeMember)
                {
                    foreach (GenMember member in _memberList)
                        val += member._type.ValueCount;
                }
                else val = -1;
                return val;
            }
        }

        public override string FullName { get { return "struct " + Name; } }

        internal GenStructure(bool hasDynamicSize = false)
        { _hasDynamicSizeMember = hasDynamicSize; }

        public GenStructure(string name, bool hasDynamicSize = false)
        {
            _name = name;
            _members = new Dictionary<string, GenMember>();
            _memberList = new List<GenMember>();
            _hasDynamicSizeMember = hasDynamicSize;
        }

        public void AddMember(GenType type, string name, UInt32 byteOffset, Int32 bitOffset = -1, Int32 bitSize = -1)
        {
            GenMember info = new GenMember(name, byteOffset, _memberList.Count, type);
            _memberList.Add(info);
            _members.Add(name, info);

            GenType memberType;
            //if (bitOffset != -1 && bitSize != -1)
            //    memberType = new GenBitfield((GenValueType)type, bitOffset, bitSize);
            //else
                memberType = type;

            Int64 calcByteSize = byteOffset + type._byteSize;
            if (_byteSize < calcByteSize)
                _byteSize = calcByteSize;
        }

        public void Finish()
        {
            _memberList.Sort((m1, m2) => m1._offset.CompareTo(m2._offset));
            int index = 0;
            foreach(GenMember member in _memberList)
            {
                member._index = index;
                index++;
            }

            if (_byteSize == -1)
            {
                if (_memberList.Count > 0)
                {
                    UInt64 offset = _memberList[_memberList.Count - 1]._offset;
                    Int64 maxEnd = 0;
                    for (int ii = _memberList.Count - 1; ii >= 0; ii--)
                    {
                        if (offset != _memberList[ii]._offset)
                            break;
                        else if ((_memberList[ii]._offset + _memberList[ii]._type._byteSize) > maxEnd)
                            maxEnd = _memberList[ii]._offset + _memberList[ii]._type._byteSize;
                    }
                }
                else _byteSize = 0;
            }
        }

        public override bool GetMember(string name, out GenMember member)
        {
            return _members.TryGetValue(name, out member);
        }

        public override bool GetMember(int index, out GenMember member)
        {
            if (index < _memberList.Count)
            {
                member = _memberList[(int)index];
            }
            else member = null;
            return member != null;
        }


        public override void AppendString(StringBuilder builder)
        {
            HashSet<GenType> subtypes = new HashSet<GenType>();
            builder.Append("Definition ");
            builder.AppendFormat("{0:X8}", _byteSize);
            builder.Append("bytes long\nstruct ");
            builder.Append(_name);
            builder.Append("\n{\n");
            foreach (KeyValuePair<string, GenMember> member in _members)
            {
                builder.AppendFormat("\t<{3:X8}_{2:X8}> {0} {1}\n", member.Value._type.Name, member.Value._name, member.Value._type._byteSize, member.Value._offset);
                subtypes.Add(member.Value._type);
            }
            builder.Append("};\n");
            foreach (GenType type in subtypes)
            {
                type.AppendString(builder);
                builder.AppendLine();
            }
        }

        public override bool IsImplicitTo(GenType other)
        {
            int equivMemCount = 0;
            if (other is GenStructure)
            {
                GenMember otherMem;
                var otherStruct = other as GenStructure;

                //find one member with the same name that is implicit
                foreach (KeyValuePair<string, GenMember> member in _members)
                {
                    if (otherStruct._members.TryGetValue(member.Key, out otherMem))
                    {
                        if (member.Value._type.IsImplicitTo(otherMem._type))
                        {
                            equivMemCount++;
                            break; //only need to find one equivalent member.  will be rechecking when comparing or overlaying.
                        }
                    }
                }
            }
            return equivMemCount > 0;
        }
    }
}
