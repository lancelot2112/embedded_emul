using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Net;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using System.Xml;
using GenericUtilitiesLib;
using EmbedEmul.Binary;
using EmbedEmul.Memory;
using EmbedEmul.SystemBus;
using EmbedEmul.SystemBus.DataBusExtensions;
using EmbedEmul.Types;
using EmbedEmul.Variables;
using Microsoft.VisualBasic.FileIO;

namespace EmbedEmul.Hardware
{
    public enum RTLOperationType
    {
        LoadFromReg,
        StoreToReg,
        REGISTER,
        LoadFromAddr,
        StoreToAddr,
        MEMORYMAPPED,
        Carry,
        Add,
        Multiply,
        ALGEBRAIC,
        ShiftRight,
        ShiftLeft,
        RotateRight,
        RotateLeft,
        BITWISE
    }

    public struct RTLOperation
    {
        public RTLOperationType Type { get; init; }
        public UInt64 Operand1 { get; init; }
        public byte Operand2 { get; init; }
        public byte Operand3 { get; init; }

        private void RegisterOperation(IDataBus registers, List<UInt64> fieldVals, Stack<UInt64> stack)
        {
            //Operand1 records the register type offset start
            //Operand2 holds the index of the field value
            //Operand3 holds the index of the field to read (store) or write (load)
            UInt64 regOffset = Operand1;
            UInt64 regIdx = fieldVals[Operand2];
            registers.BusAddress = regOffset + (regIdx << 3);

            if (Type == RTLOperationType.LoadFromReg)
            {
                stack.Push(registers.GetUInt64());
            }
            else if (Type == RTLOperationType.StoreToReg)
            {
                registers.SetUInt64(stack.Pop());
            }
            else throw new NotImplementedException();
        }

        private void SystemOperation(IDataBus system, List<UInt64> fieldVals, Stack<UInt64> stack)
        {
            //Operand1 holds the index of the address
            system.BusAddress = fieldVals[(int)Operand1];
            UInt64 value = fieldVals[Operand2];
            byte size = Operand3;
            if (Type == RTLOperationType.LoadFromAddr)
            {
                stack.Push(system.GetValue(size));
            }
            else if (Type == RTLOperationType.StoreToAddr)
            {
                system.SetValue(size, stack.Pop());
            }
            else throw new NotImplementedException();
        }

        private void AlgebraicOperation(List<UInt64> fieldVals, Stack<UInt64> stack)
        {
            UInt64 val1 = fieldVals[(int)Operand1];
            UInt64 val2 = fieldVals[Operand2];
            if (Type == RTLOperationType.Carry)
            {

            }
            else if (Type == RTLOperationType.Add)
            {
                stack.Push(val1 + val2);
            }
            else if (Type == RTLOperationType.Multiply)
            {
                stack.Push(val1 * val2);
            }
            else throw new NotImplementedException();
        }

        private void BitwiseOperation(List<UInt64> fieldVals, Stack<UInt64> stack)
        {
            UInt64 val = stack.Pop();
            int shift = (int)fieldVals[Operand2];
            int size = (int)fieldVals[Operand3];

            if (Type == RTLOperationType.ShiftLeft)
            {
                stack.Push(val << shift);
            }
            else if (Type == RTLOperationType.ShiftRight)
            {
                stack.Push(val >> shift);
            }
            else if (Type == RTLOperationType.RotateLeft)
            {
                stack.Push((val << shift) | (val >> (size - shift)));
            }
            else if (Type == RTLOperationType.RotateRight)
            {
                stack.Push((val >> shift) | (val << (size - shift)));
            }
            else throw new NotImplementedException();
        }
        public void Operate(IDataBus registers, IDataBus system, List<UInt64> fieldVals, Stack<UInt64> stack)
        {
            if (Type < RTLOperationType.REGISTER)
            {
                RegisterOperation(registers, fieldVals, stack);
            }
            else if (Type < RTLOperationType.MEMORYMAPPED)
            {
                SystemOperation(system, fieldVals, stack);
            }
            else if (Type < RTLOperationType.ALGEBRAIC)
            {
                AlgebraicOperation(fieldVals, stack);
            }
            else if (Type < RTLOperationType.BITWISE)
            {
                BitwiseOperation(fieldVals, stack);
            }
        }
    }
}