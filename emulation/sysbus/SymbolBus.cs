using System;
using System.Collections.Generic;
using System.Diagnostics;
using EmbedEmul.Types;
using EmbedEmul.Variables;

namespace EmbedEmul.SystemBus.DataBusExtensions
{
    #region INTERFACE
    public interface ISymbolBus
    {
        bool ResolvePath(string path);
        bool ResolveId(int id);
        bool NextValue();
        string SymbolPath { get; set; }
        UInt64 BusAddress { get; set; }

        UInt64 GetValue();
        string GetString();
    }
    #endregion

    #region SYMBOL BUS
    public class SymbolBus : ISymbolBus
    {
        VariableTable _symbols;
        IDataBus _data;
        Variable _symbol;
        Stack<GenMember> _stack = new Stack<GenMember>();

        public UInt64 BusAddress
        {
            get { return _data.BusAddress; }
            set { Jump(value); }
        }
        public string SymbolPath {
            get { return _symbol != null ? _symbol.Label + ((_stack.Count >= 1) ? _stack.Peek()._name : "") : ""; }
            set { ResolvePath(value); }
        }

        internal GenMember CurrentMember
        {
            get { return (_stack.Count >= 1) ? _stack.Peek() : null; }
        }
        public bool Jump(UInt64 busAddress)
        {
            _data.BusAddress = busAddress;
            _stack.Clear();
            _symbol = null;
            if (_symbols.TryGetVariableByAddress(busAddress, out _symbol))
            {
                _stack.Push(new(_symbol.Label, 0, 0, _symbol.Type));

                UInt64 memberAddress = _stack.Peek()._offset + _symbol._fileAddress;
                while (memberAddress < busAddress && NextValue())
                    memberAddress = _stack.Peek()._offset + _symbol._fileAddress;
            }
            return true;
        }
        public bool ResolvePath(string path)
        {
            int varIdx;
            _symbol = null;
            _stack.Clear();
            int index = path.IndexOfAny(GenType.PathChars);
            if (_symbols._variableByLabel.TryGetValue(path.Substring(0, index), out varIdx))
            {
                _symbol = _symbols._variables[varIdx];
                if (_symbol._type.ResolvePath(path, out GenMember member))
                {
                    _stack.Push(member);
                }
            }
            //else
            //{
            //List<int> varIndices;
            //if (_symbols._staticVariableByLabel.TryGetValue(symbol, out varIndices))
            //{

            //}
            //}

            return _symbol != null;
        }

        public string GetString()
        {
            GenMember member = CurrentMember;
            if (member != null)
            {
                _data.BusAddress = _symbol._fileAddress + member._offset;
                var baseVal = (GenBaseValue)member._type;
                //baseVal.GetString()

            }
            return "";
        }
        public UInt64 GetValue()
        {
            GenMember member = CurrentMember;
            if (member != null)
            {
                _data.BusAddress = _symbol._fileAddress + member._offset;
                return _data.GetValue((byte)member._type._byteSize);
            }
            else return 0;
        }

        public bool Deref()
        {
            GenMember member = CurrentMember;
            if (member != null && member._type is GenPointer ptr)
            {
                _data.BusAddress = _symbol._fileAddress + member._offset;
                UInt64 address = _data.GetValue((byte)member._type._byteSize);
                _data.BusAddress = address;
                _stack.Push(new(ptr.PointerType._name, 0, 0, ptr.PointerType));
                return true;
            }
            else return false;
        }
        public bool NextValue()
        {
            GenMember current = _stack.Peek();
            GenMember nextMember = null;
            int lastIndex = -1;
            int nextIndex = lastIndex + 1;
            if (_stack.Count > 1 && current._type is GenBaseValue)
            {
                _stack.Pop();
                lastIndex = current._index;
                current = _stack.Peek();

                //Walk back up the tree
                nextIndex = lastIndex + 1;
                while (_stack.Count > 1 && !current._type.GetMember(nextIndex, out nextMember))
                {
                    _stack.Pop();
                    current = _stack.Peek();
                    nextIndex = current._index + 1;
                }
            }

            //Walk down the tree
            if (nextMember == null || !(nextMember._type is GenBaseValue))
            {
                nextIndex = lastIndex + 1;
                while (current._type.GetMember(nextIndex, out nextMember) && !(nextMember._type is GenBaseValue))
                {
                    _stack.Push(nextMember);
                    nextIndex = nextMember._index;
                }
            }

            if (nextMember != null)
                _stack.Push(nextMember);

            return nextMember != null;
        }

        public bool ResolveId(int id)
        {
            int varIdx;
            _symbol = null;
            if (_symbols._variableById.TryGetValue(id, out varIdx))
            {
                _symbol = _symbols._variables[varIdx];
            }
            return _symbol != null;
        }

    }
    #endregion
}