using GenericUtilitiesLib;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Text;
using System.Threading.Tasks;

namespace EmbedEmul.Types
{
    public class GenArray : GenType, IGenDynamicSize
    {
        internal GenType _member;
        public GenType Member { get { return _member; } }
        internal Int64 _startIndex;
        internal Int64 _maxCount;
        //Dynamic size implies first two bytes (or first element if larger than two bytes is unsigned size)
        public bool IsDynamicSize
        {
            get { return _isDynamicSize; }
            set { _isDynamicSize = value; }
        }
        internal bool _isDynamicSize;

        public override Int64 ValueCount { get { return _isDynamicSize ? -1 : _maxCount * _member.ValueCount; } }

        public override string Name { get { return _member != null ? _member._name : "void"; } }
        public override string FullName
        {
            get
            {
                var builder = ObjectFactory.StringBuilders.GetObject();
                builder.Clear();

                builder.Append(Name);
                GenArray subArray = _member as GenArray;
                while (subArray != null)
                {
                    builder.Append("[");
                    if (subArray._isDynamicSize)
                        builder.Append("unknown");
                    else
                        builder.Append(subArray._maxCount);
                    builder.Append("]");
                }
                builder.Append("[");
                if (_isDynamicSize)
                    builder.Append("unknown");
                else
                    builder.Append(_maxCount);
                builder.Append("]");

                string returnVal = builder.ToString();
                ObjectFactory.StringBuilders.ReleaseObject(builder);
                return returnVal;
            }
        }

        internal GenArray(bool dynamicSize = false)
        { _isDynamicSize = dynamicSize; }
        public GenArray(string name, bool dynamicSize = false)
        {
            _name = name;
            _isDynamicSize = dynamicSize;
        }
        public GenArray(string name, GenType memberType, Int64 startIndex, Int64 endIndex, Int64 byteSize = -1, bool dynamicSize = false)
        {
            _name = name;
            _isDynamicSize = dynamicSize;
            SetMemberType(memberType, startIndex, endIndex, byteSize);
        }

        public void SetMemberType(GenType memberType, Int64 startIndex = 0, Int64 endIndex = 0, Int64 byteSize = -1)
        {
            _startIndex = startIndex;
            _maxCount = endIndex - startIndex + 1;
            _member = memberType;

            if (byteSize == -1)
                _byteSize = memberType._byteSize * (long)(endIndex - startIndex + 1);
            else
                _byteSize = byteSize;

            if (string.IsNullOrEmpty(_name))
                _name = memberType.Name;
        }

        public void SetCount(Int64 count)
        {
            _isDynamicSize = false;
            _maxCount = count;
            _byteSize = count * _member._byteSize;
        }

        public override void SetByteSize(Int64 byteSize)
        {
            _isDynamicSize = false;
            _byteSize = byteSize;
            _maxCount = _byteSize / _member._byteSize;
        }

        public override bool GetMember(string name, out GenMember member)
        {
            if (int.TryParse(name, out int idx) && idx < _maxCount)
            {
                return GetMember(idx, out member);
            }
            else member = null;
            return member != null;
        }

        public override bool GetMember(int index, out GenMember member)
        {
            if (index < _maxCount)
            {
                member = new GenMember($"[{index}]", (uint)(index * _member.ByteSize), (int)index, _member);
            }
            else member = null;
            return member != null;
        }

        public override void AppendString(StringBuilder builder)
        {
            builder.Append(Name);

            GenArray subArray = _member as GenArray;
            while (subArray != null)
            {
                builder.Append("[");
                if (subArray._isDynamicSize)
                    builder.Append("unknown");
                else
                    builder.Append(subArray._maxCount);
                builder.Append("]");
            }
            builder.Append("[");
            if (_isDynamicSize)
                builder.Append("unknown");
            else
                builder.Append(_maxCount);
            builder.Append("];\n");

            _member.AppendString(builder);
        }

        public override bool IsImplicitTo(GenType other)
        {
            bool equivalent = true;
            if (other is GenArray)
            {
                var otherArr = other as GenArray;
                equivalent = otherArr._maxCount == _maxCount && otherArr._member.IsImplicitTo(_member);
            }
            else equivalent = false;
            return equivalent;
        }
    }
}
