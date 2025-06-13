using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Tools;
using GenericUtilitiesLib;

namespace EmbedEmul.Types
{
    public interface IGenDynamicSize
    {
        bool IsDynamicSize { get; }
    }

    public interface IMemberAccess
    {
        bool GetMember(int index, out GenMember type);
        bool GetMember(string name, out GenMember type);
        bool NextMember(string prevName, out GenMember type);
    }

    /// <summary>
    /// Parent class
    /// </summary>
    public abstract class GenType : IMemberAccess
    {
        internal static UInt32 _ID_ = 0;

        internal string _name;
        public virtual string Name { get { return _name; } }
        public virtual string FullName { get { return Name; } }

        internal Int64 _byteSize;
        public Int64 ByteSize { get { return _byteSize; } set { SetByteSize(value); } }

        public virtual Int64 ValueCount { get { return 1; } }

        public virtual void SetByteSize(Int64 byteSize)
        {
            Debug.Assert(byteSize != 0);
            _byteSize = byteSize;
        }

        internal UInt32 _id;

        public GenType()
        {
            _id = _ID_++;
        }
        /// <summary>
        /// Build a value from the bytes in data and store as current value
        /// </summary>
        /// <param name="data"></param>
        //public abstract IEnumerable<GenValue> Image(BinaryBlock data);
        /// <summary>
        /// Extract byte representation of the current value and append to the supplied byte data at the current index
        /// </summary>
        /// <param name="data"></param>
        //public abstract void GetBytes(ByteData data);

        //public abstract GenValue GetValue(string symbolName);

        //public abstract IEnumerable<GenValue> GetValue(UInt32 offset);

        //public abstract void WriteValue(GenType value);

        //public abstract void ValuesAtOffset(UInt32 offset, List<GenValue> values);

        public virtual void AppendString(StringBuilder builder)
        {
            builder.AppendFormat("{0} Size:{1}\n", _name, _byteSize);
        }
        public override string ToString()
        {
            var builder = ObjectFactory.StringBuilders.GetObject();
            builder.Clear();
            AppendString(builder);
            string result = builder.ToString();
            ObjectFactory.StringBuilders.ReleaseObject(builder);
            return result;
        }

        public virtual bool GetMember(string name, out GenMember member)
        {
            if (name == "" || name == null)
                member = new GenMember(_name, 0, 0, this);
            else member = null;
            return member != null;
        }

        public virtual bool GetMember(int index, out GenMember member)
        {
            if (index <= 0)
                member = new GenMember(_name, 0, 0, this);
            else member = null;
            return member != null;
        }

        public virtual bool NextMember(string prevName, out GenMember member)
        {
            if (GetMember(prevName, out var prevMember))
            {
                return GetMember(prevMember._index + 1, out member);
            }
            else member = null;
            return member != null;
        }

        public static char[] PathChars = new char[] { '[', '.', ':' };
        public bool ResolvePath(string path, out GenMember member)
        {
            string[] pathParts = path.Remove(']').Split(PathChars);
            member = new GenMember(path, 0, 0, null);
            for(int ii = 0; ii< pathParts.Length; ii++)
            {
                if (GetMember(pathParts[ii], out var tmpMember))
                {
                    member._index = tmpMember._index;
                    member._offset += tmpMember._offset;
                    member._type = tmpMember._type;
                }
                else
                {
                    //TODO: Some error checking or notification?
                    member = null;
                    break;
                }

            }
            return member != null;
        }

        /// <summary>
        /// Implies that one type can be transformed into another
        /// ::Value Criteria::
        /// Values must both be strings or must both be numeric (all numeric types can be implicitly cast in this framework)
        /// ::Array Criteria::
        /// Member types must be implicit and both must have the same number of elements
        /// ::Struct Criteria::
        /// if type names are the same at least one member that share the same label must be implicit
        /// </summary>
        /// <param name="other"></param>
        /// <returns></returns>
        public virtual bool IsImplicitTo(GenType other)
        {
            return other.GetType() == this.GetType();
        }

        /// <summary>
        /// Implies that each are just a different representation of the same bytes allows
        /// a virtual union type operation
        /// </summary>
        /// <param name="other"></param>
        /// <returns></returns>
        public virtual bool IsExplicitTo(GenType other)
        {
            return other._byteSize == _byteSize;
        }
    }
}
