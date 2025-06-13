using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Binary;
using EmbedEmul.Memory;

namespace EmbedEmul.Programs.TIS.Dwarf
{
    public class DWOperation
    {
        internal byte _operation;
        public DWOpType OpType { get { return (DWOpType)_operation; } }

        internal UInt64 _value;

        public DWOperation(MemoryUnit data)
        {
            _operation = data.GetUInt8();

            if(OpType == DWOpType.DW_OP_addr ||
                OpType == DWOpType.DW_OP_const ||
                OpType == DWOpType.DW_OP_reg ||
                OpType == DWOpType.DW_OP_breg)
            {
                _value = data.GetUInt32();
            }
            else if (OpType == DWOpType.DW_OP_plus_uconst)
            {
                _value = data.GetULEB128();
            }
        }

        public void Operate(Stack<UInt64> stack)
        {
            if (OpType == DWOpType.DW_OP_const ||
                OpType == DWOpType.DW_OP_addr)
                stack.Push(_value);
            else if (OpType == DWOpType.DW_OP_add)
            {
                UInt64 val = stack.Pop();
                stack.Push(val + stack.Peek());
            }
            else if (OpType == DWOpType.DW_OP_plus_uconst)
            {
                stack.Push(_value + stack.Peek());
            }
            else
                throw new NotImplementedException();
        }

        public override string ToString()
        {
            return string.Format("{0}(0x{1:X8})", OpType, _value);
        }
    }
}
