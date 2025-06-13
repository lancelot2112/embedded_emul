using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using GenericUtilitiesLib;
using EmbedEmul.Binary;
using EmbedEmul.Memory;

namespace EmbedEmul.Programs.TIS.Dwarf
{
    public class DWExpression
    {
        public static ObjectCache<Stack<UInt64>> StackCache = new ObjectCache<Stack<UInt64>>();
        private void ReleaseStack(Stack<UInt64> stack)
        {
            stack.Clear();
            StackCache.ReleaseObject(stack);
        }
        List<DWOperation> _operations;
        public List<DWOperation> Value { get { return _operations; } }

        public DWExpression(MemoryUnit data)
        {
            _operations = new List<DWOperation>();
            while (!data.EndOfRange)
                _operations.Add(new DWOperation(data));
        }

        public UInt64 Operate(UInt64 input)
        {
            var stack = StackCache.GetObject();
            stack.Push(input);
            foreach (DWOperation op in _operations)
                op.Operate(stack);
            UInt64 result = stack.Peek();
            ReleaseStack(stack);
            return result;
        }

        public override string ToString()
        {
            StringBuilder build = new StringBuilder();
            foreach (DWOperation op in _operations)
                build.AppendFormat("{0} ", op);

            return build.ToString();
        }
    }
}
