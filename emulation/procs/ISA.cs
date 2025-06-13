using System;
using System.Collections;
using System.Collections.Generic;
using System.Collections.ObjectModel;
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
    public enum ISACmdToken
    {
        Parameter,
        MemorySpace,
        Field,
        Instruction
    }

    public record ISAParameter(string Label, string Value);

    public enum ISASpaceType
    {
        Memory,
        Registers,
        MemoryMappedIO
    }
    public record ISASpace
    (
        string Label,
        Endianness Endianness,
        UInt32 Alignment,
        UInt32 WordSize,
        UInt32 AddressSize,
        ISASpaceType Type
    );

    [Flags]
    public enum ISAFieldType
    {
        None = 0,
        Immediate = 1 << 0,
        Exts = 1 << 1,
        Extz = 1 << 2,
        Register = 1 << 3,
        Address = 1 << 4,
        Source = 1 << 5,
        Target = 1 << 6,
        FuncCode
    }
    public record InsnField
    (
        string Label,
        string Postfix,
        BitSlice[] Slices, //Ranges appended together b001 b1111->b0011111
        ISAFieldType Type //Whether or not this is signed (MostSigBit is sign bit)
    )
    {
        public UInt64 Decode(UInt64 instruction)
        {
            UInt64 value = 0;
            UInt16 totalSize = 0;
            foreach (var slice in Slices)
            {
                value = (value << slice.Size) | ((instruction & slice.Mask) >> slice.Shift);
                totalSize += slice.Size;
            }
            value = (Type & ISAFieldType.Exts) > 0 ? Exts(value, totalSize) : value;
            return value;
        }

        private UInt64 Exts(UInt64 value, UInt16 size)
        {
            //Sign extending algorithm
            UInt64 sgn_mask = (UInt64)((1 << size) - 1);
            sgn_mask = (sgn_mask >> 32) | (sgn_mask >> 16) | (sgn_mask >> 8) | (sgn_mask >> 4) | (sgn_mask >> 2) | (sgn_mask >> 1);
            if ((value & (sgn_mask ^ (sgn_mask >> 1))) > 0)
                value |= (~sgn_mask);
            return value;
        }
    }

    public class ISAFields
    {
        UInt16 ClassSize; //0-(ClassSize-1) for bit numbering
        List<object> Fields;
    }

    internal class ISAOpCodeGroup
    {
        internal Dictionary<UInt64, ISAInstructionPrototype> Instructions;
    }



    public class ISAInstructionPrototype
    {
        public static ISAInstructionPrototype NOP = new ISAInstructionPrototype("NOP");
        public string Label;
        public List<InsnField> Fields;
        public List<RTLOperation> RTL;

        public ISAInstructionPrototype(string label)
        {
            Label = label;
        }

        public virtual bool Operate(IDataBus registers, IDataBus system, UInt64 bits, List<UInt64> fieldVals, Stack<UInt64> stack)
        {
            return false;
            foreach (var field in Fields)
            {
                fieldVals.Add(field.Decode(bits));
            }

            foreach (var op in RTL)
            {
                op.Operate(registers, system, fieldVals, stack);
            }
        }

        public virtual string Disasm(UInt64 bits)
        {
            StringBuilder builder = new StringBuilder();
            builder.Append(Label);
            builder.Append(" ");

            foreach (var field in Fields)
            {
                UInt64 value = field.Decode(bits);
                builder.Append(value.ToString("X"));
                builder.Append(" ");
                //field.Display(value, ref builder);
            }
            builder.AppendLine();
            return builder.ToString();
        }
        public virtual UInt64 Asm(string asm) { return 0; }
    }

    public class ISAPrototype
    {
        Endianness Endianness;
        UInt32 Alignment;
        ISASpace RegisterSpace;
        ISASpace MemorySpace;
        RegisterTable RegisterTable;
        Dictionary<UInt64, ISAOpCodeGroup> OpCodeGroups;
        Dictionary<string, string> Params;
        Dictionary<string, ISASpace> Spaces;
        Dictionary<string, InsnField> Fields;

        public ISAPrototype()
        {
            OpCodeGroups = new Dictionary<UInt64, ISAOpCodeGroup>();
            Params = new Dictionary<string, string>();
            Spaces = new Dictionary<string, ISASpace>();
            RegisterTable = new RegisterTable();
            Fields = new Dictionary<string, InsnField>();
        }

        public void InitCoreRegisters(RegisterBus registerBus)
        {

        }

        public bool MatchInstruction(UInt64 opCode, UInt64 function, out ISAInstructionPrototype insn)
        {
            insn = ISAInstructionPrototype.NOP;
            if (!OpCodeGroups.TryGetValue(opCode, out var group))
                return false;

            return group.Instructions.TryGetValue(function, out insn);
        }
        public void Read(string path)
        {
            ISASpace currentSpace;
            var info = new FileInfo(path);
            using (var file = info.OpenRead())
            using (var reader = new TextFieldParser(file))
            {
                reader.SetDelimiters(" ");
                reader.CommentTokens = new string[] { "#" };
                reader.TextFieldType = FieldType.Delimited;
                reader.HasFieldsEnclosedInQuotes = true;

                while (!reader.EndOfData)
                {
                    string[] fields = reader.ReadFields();
                    //Remove inline comments
                    if (fields == null || fields.Length == 0)
                        continue;

                    //Lines that start with a : indicate an operation of some kind
                    if (fields[0].StartsWith(':'))
                    {
                        switch (fields[0])
                        {
                            case ":param":
                                string[] vals = fields[1].Split('=');
                                Params.Add(vals[0], vals[1]);
                                break;
                            case ":space":
                                ParseSpace(fields);
                                break;
                            case ":fields":
                                ParseFields(fields, reader);
                                break;
                            case ":macro":
                                throw new NotImplementedException("Macros not implemented yet.");
                            //break;
                            case ":insn":
                                ParseInstruction(fields, reader);
                                break;
                            default:
                                if (Spaces.TryGetValue(fields[0].Substring(1), out currentSpace))
                                {
                                    //This is a named memory range
                                    if (currentSpace.Type == ISASpaceType.Registers)
                                    {
                                        ParseRegisterFile(fields, reader);
                                    }
                                    else throw new NotImplementedException($"Not expecting space input {currentSpace.Label}");
                                }
                                break;
                        }
                    }
                }
            }
        }

        private void ParseInstruction(string[] opts, TextFieldParser reader)
        {
            string name = opts[1];
            string[] fields = opts[2].Trim('(', ')').Split(',');
            string descr = null;

            for (int ii = 3; ii < opts.Length; ii++)
            {
                if (opts[ii].StartsWith("mask"))
                {
                    string[] first = opts[ii].Split("={");

                    List<string> masks = new List<string>();
                    if (first[1].Contains('}'))
                    {
                        masks.Add(first[1].Trim('}'));
                    }
                    else
                    {
                        masks.Add(first[1]);
                        while (++ii < opts.Length && !opts[ii].Contains('}'))
                        {
                            masks.Add(opts[ii]);
                        }
                        masks.Add(opts[ii].Trim('}'));
                    }

                    foreach (var mask in masks)
                    {
                        string[] defn = mask.Split('=');
                        if (defn[0].StartsWith('@')) //Unnamed bit range
                        {
                            var slices = ParseBitDefinition(32, defn[0]);
                            if (slices.Count > 1)
                                throw new NotImplementedException();

                            var slice = slices[0];
                            slice.Mask = ulong.Parse(defn[1]);
                        }
                        else if (Fields.TryGetValue(defn[0], out var field)) //named bit range
                        {

                        }
                        else throw new InvalidDataException($"Not expecting mask '{mask}'");
                    }
                }
                else
                {
                    string[] values = opts[ii].Split("=");
                    switch (values[0].ToLower())
                    {
                        case "descr":
                            descr = values[1];
                            break;
                        default:
                            throw new InvalidDataException($"Was not expecting {opts[ii]} in :insn {name}");
                    }
                }
            }
        }

        private void ParseRegisterFile(string[] opts, TextFieldParser reader)
        {
            string className = opts[1];
            UInt64 offset = 0;
            UInt16 bitSize = (ushort)RegisterSpace.WordSize;
            UInt16 count = 1;
            string nameForm = null;
            UInt64 reset = 0;
            string descr = "";
            string alias = null;
            for (int ii = 2; ii < opts.Length; ii++)
            {
                string[] value = opts[ii].Split('=');
                switch (value[0])
                {
                    case "offset":
                        offset = ulong.Parse(value[1]);
                        break;
                    case "size":
                        bitSize = ushort.Parse(value[1]);
                        break;
                    case "count":
                        count = ushort.Parse(value[1]);
                        break;
                    case "name":
                        nameForm = value[1];
                        break;
                    case "reset":
                        reset = ulong.Parse(value[1]);
                        break;
                    case "descr":
                        descr = value[1];
                        break;
                    case "alias":
                        alias = value[1];
                        break;
                    default:
                        throw new NotSupportedException($"Unexpected field {value[0]} in register file parse.");

                }
            }

            RegisterFile file;
            if (alias == null)
            {
                var field = new RegisterField(className, descr, reset, BitSlice.CreateSlice(bitSize, 0, (ushort)(bitSize - 1)));
                file = new RegisterFile()
                {
                    Base = field,
                    Offset = offset,
                    Count = count,
                    NameFormat = nameForm
                };
                if (!RegisterTable.Register(file))
                    throw new InvalidDataException($"Register file '{className}' already exists.");
            }
            else
            {
                var field = new RegisterField(className, descr, reset, BitSlice.Zero);
                file = new RegisterFile()
                {
                    Base = field,
                    Count = 1
                };
                if (!RegisterTable.Alias(file, alias))
                    throw new InvalidDataException($"Register alias '{alias}' already exists.");
            }

            ParseRegFields(file, reader);
        }

        private void ParseRegFields(RegisterFile file, TextFieldParser reader)
        {
            while (reader.PeekChars(1) != ":")
            {
                string[] regFieldOpts = reader.ReadFields();
                if (regFieldOpts == null || regFieldOpts.Length < 2)
                    continue;

                string name = regFieldOpts[0];
                var slices = ParseBitDefinition((ushort)RegisterSpace.WordSize, regFieldOpts[1]);

                if (slices.Count != 1) throw new NotImplementedException();

                string descr = null;
                UInt64 reset = 0;
                for (int ii = 2; ii < regFieldOpts.Length; ii++)
                {
                    string[] value = regFieldOpts[ii].Split('=');
                    switch (value[0].ToLower())
                    {
                        case "descr":
                            descr = value[1];
                            break;
                        case "reset":
                            reset = ulong.Parse(value[1]);
                            break;
                        default:
                            throw new NotImplementedException($"Not expecting option {regFieldOpts[ii]}");
                    }
                }

                if (file.Fields == null)
                    file.Fields = new Dictionary<string, RegisterField>();

                file.Fields.Add(name, new RegisterField(name, descr, reset, slices[0]));
            }
        }
        private void ParseFields(string[] opts, TextFieldParser reader)
        {
            ushort classSize = 32; //Default class size
            if (opts.Length > 1 && opts[1].StartsWith("size"))
            {
                string[] sizeParts = opts[1].Split('=');
                if (sizeParts.Length != 2 || !ushort.TryParse(sizeParts[1], out classSize))
                    throw new NotSupportedException($"Invalid size definition: {opts[1]}");
            }
            string openBrace = reader.ReadLine().Trim();
            if (!openBrace.StartsWith("{"))
                throw new NotSupportedException("Expected '{' after field definition.");

            while (!reader.EndOfData)
            {
                string line = reader.ReadLine().Trim();
                if (line.StartsWith("}"))
                    break; //End of field definitions

                if (string.IsNullOrWhiteSpace(line) || line.StartsWith("#"))
                    continue; //Skip empty lines and comments

                string[] fieldParts = line.Split(' ').Where(s => !string.IsNullOrWhiteSpace(s)).ToArray();
                if (fieldParts.Length < 2)
                    throw new NotSupportedException($"Invalid field definition: {line}");

                string[] labelParts = fieldParts[0].Split('?');
                ISAFieldType fieldType = ISAFieldType.None;
                string label = labelParts[0];
                string option = "";
                if (labelParts.Length > 1)
                {
                    //This is a postfix to add to the name of a function
                    //This must be a function code.
                    option = labelParts[1];
                    fieldType = ISAFieldType.FuncCode;
                }

                var slices = ParseBitDefinition(classSize, fieldParts[1]);

                string[] type = fieldParts[2].Split('=');
                if (type[0] != "op")
                    throw new InvalidDataException($"No field operation defined {label}");


                foreach (string op in type[1].Split('|'))
                {
                    string[] args = op.Split('.');
                    fieldType |= args[0].ToLower() switch
                    {
                        "func" => ISAFieldType.FuncCode,
                        "reg" => ISAFieldType.Register,
                        "exts" => ISAFieldType.Exts,
                        "extz" => ISAFieldType.Extz,
                        "imm" => ISAFieldType.Immediate,
                        "addr" => ISAFieldType.Address,
                        _ => throw new NotSupportedException($"Unexpected field type {args[0]}")
                    };

                    if (args[0].ToLower() == "reg")
                    {
                        if (args.Length == 1)
                            option = "GPR";
                        else option = args[1];
                    }
                }

                Fields.Add(label, new InsnField(label, option, slices.ToArray(), fieldType));
            }
        }

        private List<BitSlice> ParseBitDefinition(UInt16 classSize, string bitDefn)
        {
            if (!bitDefn.StartsWith('@'))
                throw new NotSupportedException($"Invalid bit definition '{bitDefn}'");

            StringBuilder bitNumber = new StringBuilder();
            List<BitSlice> slices = new List<BitSlice>();
            int start = -1;
            int end = -1;
            int state = 0; //0=none, 1=bitRange, 2=pad count
            for (int ii = 1; ii < bitDefn.Length; ii++)
            {
                //Check for exit conditions on things that may immediately transition
                //or don't have an exit character
                if (state == 2)
                {
                    if (bitDefn[ii] != '0' || bitDefn[ii] != '1')
                    {
                        state = 0;
                        UInt64 mask = Convert.ToUInt64(bitNumber.ToString(), 2);
                        slices.Add(BitSlice.CreatePad(mask, (ushort)bitNumber.Length, 0));
                    }
                    else bitNumber.Append(bitDefn[ii]);
                }

                if (state == 0)
                {
                    if (bitDefn[ii] == 'b') //padCount Start
                    {
                        bitNumber.Clear();
                        state = 2;
                    }
                    else if (bitDefn[ii] == '(') //Start of a bit range
                    {
                        bitNumber.Clear();
                        start = -1;
                        end = -1;
                        state = 1;
                    }
                    //Ignore extra stuff
                }
                else if (state == 1)
                {
                    //check for closing of span
                    if (bitDefn[ii] == ')')
                    {
                        end = Convert.ToUInt16(bitNumber.ToString(), 10);
                        if (start == -1)
                            slices.Add(BitSlice.CreateFlag(classSize, (ushort)end));
                        else slices.Add(BitSlice.CreateSlice(classSize, (ushort)start, (ushort)end));
                        state = 0;
                    } //check for a start-end construct
                    else if (bitDefn[ii] == '-')
                    {
                        start = Convert.ToUInt16(bitNumber.ToString(), 10);
                        bitNumber.Clear();
                    }
                    else if (char.IsNumber(bitDefn[ii]))
                    {
                        bitNumber.Append(bitDefn[ii]);
                    }
                    else throw new InvalidDataException($"Not expecting char '{bitDefn[ii]}' in {bitDefn}");
                }
            }
            return slices;
        }

        private void ParseSpace(string[] opts)
        {
            string name = opts[1];
            uint addrSize = 32;
            uint wordSize = 32;
            uint align = 8;
            Endianness endian = Endianness.Little;
            ISASpaceType type = 0; //0=memory, 1=registers, 2=memory mapped
            for (int ii = 2; ii < opts.Length; ii++)
            {
                string[] value = opts[ii].Split('=');
                switch (value[0])
                {
                    case "addr":
                        addrSize = uint.Parse(value[1]);
                        break;
                    case "word":
                        wordSize = uint.Parse(value[1]);
                        break;
                    case "align":
                        align = uint.Parse(value[1]);
                        break;
                    case "endian":
                        endian = value[1].ToLower() switch
                        {
                            "little" => Endianness.Little,
                            "big" => Endianness.Big,
                            _ => throw new NotSupportedException($"Unknown endianness: {value[1]}")
                        };
                        break;
                    case "type":
                        type = value[1].ToLower() switch
                        {
                            "memory" => ISASpaceType.Memory,
                            "registers" => ISASpaceType.Registers,
                            "memorymapped" => ISASpaceType.MemoryMappedIO,
                            _ => throw new NotSupportedException($"Unknown space type: {value[1]}")
                        };
                        break;
                    default:
                        throw new NotSupportedException($"Unknown space parameter: {value[0]}");
                }
            }

            ISASpace space = new ISASpace(name, endian, align, wordSize, addrSize, type);
            Spaces.Add(name, space);

            if (type == ISASpaceType.Registers)
            {
                RegisterSpace = space;
            }
            else if (type == ISASpaceType.Memory)
            {
                MemorySpace = space;
            }
            else
            {
                throw new NotSupportedException($"Unsupported space type: {type}");
            }
        }
    }
}