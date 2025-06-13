using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EmbedEmul.Types
{
    public class GenPointer : GenBaseValue
    {
        public GenType PointerType;

        public override string Name { get { if (_name == null) Finish(); return _name; } }

        public GenPointer(GenType type, Int64 byteSize = 4)
            : base(null, byteSize, ValueEncoding.Unsigned)
        {
            SetType(type);
            _displayFormat = DisplayFormat.hex;
        }

        public void SetType(GenType type)
        {
            PointerType = type;
        }

        public void Finish()
        {
            if (PointerType == null)
                _name = "void*";
            else if (PointerType is GenSubroutine)
            {
                var subType = PointerType as GenSubroutine;
                _name = string.Format("{0} ({1}*)({2})", subType.Outputs, subType.Name, subType.Inputs);
            }
            else
                _name = PointerType.Name + "*";

        }

        public override void AppendString(StringBuilder builder)
        {
            builder.AppendLine(Name);
            if (PointerType != null)
                PointerType.AppendString(builder);
        }
    }
}
