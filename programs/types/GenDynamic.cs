using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Binary;
using EmbedEmul.Memory;
using EmbedEmul.Tools;
using EmbedEmul.Variables;

namespace EmbedEmul.Types
{
    public enum GenOperationType
    {
        //Operations that add 1 to stack size
        UConst, //push UConst to stack
        OpAddressOf, //Pushes address to stack
        OpSizeOf, //Pushes size of element to stack
        OpReadUnsigned, //Pushes unsigned value of current element to stack
        OpCountOfId, //Gets the number of elements from an array
        OpEvaluateExpression, //Evaluates supplied expression (expression nesting)

        //Operations that maintain stack size
        OpAdd, //Pops 2 values - adds - push result
        OpDivide, //Pops 2 values - divides - push result
        OpNegate, //Pops value - negates - push result
        OpSubtract,
        OpAddUConst, //Pops value - Adds const - push result

        OpToggleState,
        OpMemberByName, //access type member specified by previous const and push type id to stack
        OpMemberByIndex, //access array element
        OpDerefPointer, //deref pointer at location
        OpPushVariableById, //Pushes variable to top of typed binary data stack

        OpPop, //Pops top value from typed binary data stack
        OpPopVariable, //Pops variable from typed binary data stack
    }
    //TODO: Create Metadata structure?

    [StructLayout(LayoutKind.Explicit)]
    public struct StackValue
    {
        [FieldOffset(0)]
        internal UInt64 u;
        [FieldOffset(0)]
        internal double d;
        [FieldOffset(0)]
        internal float f;

        //public StackValue AddUnsigned(StackValue other)
        //{

        //}
    }

    public class GenOperation
    {
        internal static GenOperation OpReadUnsigned = new GenOperation(GenOperationType.OpReadUnsigned);
        internal static GenOperation OpPop = new GenOperation(GenOperationType.OpPop);
        internal static GenOperation OpPopVariable = new GenOperation(GenOperationType.OpPopVariable);
        internal static GenOperation OpAdd = new GenOperation(GenOperationType.OpAdd);
        internal static GenOperation OpSubtract = new GenOperation(GenOperationType.OpSubtract);
        internal static GenOperation OpNegate = new GenOperation(GenOperationType.OpNegate);
        internal static GenOperation OpDivide = new GenOperation(GenOperationType.OpDivide);
        internal static GenOperation OpSizeOf = new GenOperation(GenOperationType.OpSizeOf);
        internal static GenOperation OpAddressOf = new GenOperation(GenOperationType.OpAddressOf);

        internal GenOperationType _opType;
        internal object _argument;

        public GenOperation(GenOperationType type, object argument = null)
        {
            _opType = type;
            _argument = argument;
        }

        public void Operate(TypedMemory data, Stack<StackValue> stack)
        {
            switch (_opType)
            {
                case GenOperationType.UConst:
                    stack.Push(new StackValue() { u = (UInt64)_argument });
                    break;
                case GenOperationType.OpAdd:
                    stack.Push(new StackValue() { u = stack.Pop().u + stack.Peek().u });
                    break;
                case GenOperationType.OpDivide:
                    stack.Push(new StackValue() { u = stack.Pop().u / stack.Peek().u });
                    break;
                case GenOperationType.OpNegate:
                    stack.Push(new StackValue() { u = (UInt64)(-(Int64)stack.Pop().u) });
                    break;
                case GenOperationType.OpSubtract:
                    stack.Push(new StackValue() { u = (UInt64)(-(Int64)stack.Pop().u + (Int64)stack.Peek().u) });
                    break;
                case GenOperationType.OpAddUConst:
                    stack.Push(new StackValue() { u = stack.Pop().u + (UInt64)_argument });
                    break;
                case GenOperationType.OpToggleState:
                    if (_argument == null)
                        _argument = data.WorkingState;
                    else
                    {
                        var value = (TypedMemory.StateEntry)_argument;
                        _argument = null;
                        var newState = data.Push(value._type, value._address._start, value._index);
                    }
                    break;
                case GenOperationType.OpMemberByName:
                    data.ViewMember((string)_argument);
                    if (!data.IsValid) throw new Exception();
                    break;
                case GenOperationType.OpMemberByIndex:
                    data.ViewMember((Int32)_argument);
                    if (!data.IsValid) throw new Exception();
                    break;
                case GenOperationType.OpDerefPointer:
                    data.Deref();
                    if (!data.IsValid) throw new Exception();
                    break;
                case GenOperationType.OpPushVariableById:
                    data.PushVar((Int64)_argument);
                    if (!data.IsValid) throw new Exception();
                    break;
                case GenOperationType.OpPop:
                    data.Pop();
                    break;
                case GenOperationType.OpPopVariable:
                    data.PopVar();
                    break;
                case GenOperationType.OpSizeOf:
                    stack.Push(new StackValue() { u = (UInt64)data.Peek()._type._byteSize });
                    break;
                case GenOperationType.OpReadUnsigned:
                    stack.Push(new StackValue() { u = data.GetUnsigned() });
                    break;
                case GenOperationType.OpCountOfId: //Need to be able to stack variables?
                    Variable variable;
                    if (data.TryGetVariable((Int64)_argument, out variable))
                    {
                        if (variable._type is GenArray)
                            stack.Push(new StackValue() { u = (UInt64)(variable._type as GenArray)._maxCount });
                        else stack.Push(new StackValue() { u = (UInt64)variable._type._byteSize });
                    }
                    else stack.Push(new StackValue() { u = 0 });
                    break;
                case GenOperationType.OpEvaluateExpression:
                    stack.Push(new StackValue() { u = (_argument as GenExpression).Resolve(data) });
                    break;
                default:
                    throw new NotImplementedException();
            }
        }
    }

    public class GenExpression
    {
        internal List<GenOperation> _ops;
        //TODO: Convert stack to array for quicker operation
        internal Stack<StackValue> _stack;
        internal UInt64 _value;

        public bool ConstantExpression { get { return _ops == null; } }

        public GenExpression(UInt64 constExpression = UInt64.MaxValue)
        {
            if (constExpression < UInt64.MaxValue)
            {
                _ops = null;
                _value = constExpression;
            }
            else
                _ops = new List<GenOperation>();
        }

        public void AddOperation(GenOperation op)
        {
            if (_ops != null)
            {
                _ops.Add(op);
                if (op._opType <= GenOperationType.OpAdd)
                    _value++;
            }
            else if (op._opType != GenOperationType.UConst)
                _value = (UInt64)op._argument;
            else
                throw new InvalidOperationException("Cannot add operation to a constant expression.");
        }

        public UInt64 Resolve(TypedMemory data, UInt64 input = 0)
        {
            if (_ops != null)
            {
                if (_stack == null)
                    _stack = new Stack<StackValue>((int)_value);

                _stack.Clear();
                _stack.Push(new StackValue() { u = input });

                foreach (GenOperation op in _ops)
                {
                    op.Operate(data, _stack);
                }

                return _stack.Peek().u;
            }
            else
            {
                return _value;
            }
        }
    }

    public enum DynamicType
    {
        ContiguousStructure,
        RuntimeDynamic
    }
    public class GenDynamic : GenType, IGenDynamicSize
    {
        public class DynamicMember
        {
            //Application specific data
            internal string _memberContainingSize;
            internal string _memberContainingCount;
            internal Int64 _variableId;
            internal GenType _type;
            internal string _label;
            internal Int32 _memberIndex;

            public DynamicMember()
            {
                _variableId = -1;
                _memberIndex = -1;
            }

            public Int32 Index { get { return _memberIndex; } }

            public override string ToString()
            {
                StringBuilder build = new StringBuilder();
                if (_label != null)
                    build.Append(_label);

                if (_type != null)
                {
                    build.Append(" ");
                    build.Append(_type.ToString());
                }

                if (_memberContainingSize != null)
                {
                    build.Append(" size:");
                    build.Append(_memberContainingSize);
                }

                if(_variableId != -1)
                {
                    build.Append(" var:");
                    build.Append(_variableId);
                }
                return build.ToString();
            }
        }

        public bool IsDynamicSize { get { return true; } }
        internal DynamicType _dynamicType;
        internal List<DynamicMember> _members;
        internal Dictionary<string, DynamicMember> _membersByLabel;
        internal Dictionary<TypedMemory, GenType> _structs;

        public GenDynamic(DynamicType type)
        {
            _dynamicType = type;

            if (_dynamicType == DynamicType.ContiguousStructure)
            {
                _members = new List<DynamicMember>();
                _membersByLabel = new Dictionary<string, DynamicMember>();
                _structs = new Dictionary<TypedMemory, GenType>();
            }
        }

        public GenType GetType(TypedMemory data, Variable var, UInt64 startAddress)
        {
            if (_dynamicType == DynamicType.RuntimeDynamic)
                throw new InvalidOperationException("Runtime dynamic requires xml declaration from ECM...");

            GenType retType;
            if(!_structs.TryGetValue(data, out retType))
            {
                GenStructure structure = new GenStructure(_name);
                long byteSize = 0;
                GenMember memberInfo;
                AddressRange memberAddress;
                Int64 value;

                foreach(DynamicMember member in _members)
                {
                    if (member._type == null)
                        throw new InvalidOperationException("Expected type definition...");

                    if (member._memberContainingSize != null && structure._members.TryGetValue(member._memberContainingSize, out memberInfo))
                    {
                        memberAddress._start = memberInfo._offset + startAddress;
                        memberAddress._length = memberInfo._type._byteSize;
                        value = data.RawMemory.GetSigned(ref memberAddress);
                        if (member._type is GenArray)
                            (member._type as GenArray)._member._byteSize = value;
                        else if (member._type is GenBaseValue)
                        {
                            var baseVal = member._type as GenBaseValue;
                            if (baseVal._encoding != ValueEncoding.String)
                            {
                                if (value > 8)
                                    throw new InvalidOperationException("value type cannot be of size greater than 8.");
                                else baseVal._byteSize = value;
                            }
                            else baseVal._byteSize = value;
                        }
                        else member._type._byteSize = value;
                    }

                    if(member._memberContainingCount != null && structure._members.TryGetValue(member._memberContainingCount, out memberInfo))
                    {
                        memberAddress._start = memberInfo._offset + startAddress;
                        memberAddress._length = memberInfo._type._byteSize;
                        value = data.RawMemory.GetSigned(ref memberAddress);
                        if (member._type is GenArray)
                            (member._type as GenArray).SetCount(value);
                    }

                    if (member._type.ByteSize > 0)
                    {
                        structure.AddMember(member._type, member._label, (uint)byteSize);
                        byteSize += member._type._byteSize;
                    }
                }
                retType = structure;
            }
            else throw new InvalidOperationException("invalid declaration."); //skip invalid member declaration

            return retType;
        }

        public void AddMember(DynamicMember member)
        {
            member._memberIndex = _members.Count;
            _members.Add(member);
            _membersByLabel.Add(member._label, member);
        }

        public void AddMember(Int64 id = -1, string label = null, GenType type = null)
        {
            var member = new DynamicMember()
            {
                _variableId = id,
                _memberIndex = _members.Count,
                _label = label,
                _type = type
            };

            if (type == null && id == -1 && label == null)
                throw new MissingFieldException("Need to define at least one of type or id");

            _members.Add(member);
            _membersByLabel.Add(member._label, member);
        }
    }
}
