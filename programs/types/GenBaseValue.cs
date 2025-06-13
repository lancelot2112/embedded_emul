using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Binary;
using EmbedEmul.Tools;
using EmbedEmul.Variables;
using GenericUtilitiesLib;
using EmbedEmul.Memory;

namespace EmbedEmul.Types
{
    /// <summary>
    /// Enumeration that numeric types that can be contained in this
    /// NumericValue
    /// </summary>
    public enum ValueEncoding : byte
    {
        Unsigned = 0,
        Signed = 1,
        Floating = 2,
        String = 3,
        None = 4
    }

    public enum DisplayFormat : byte
    {
        Default,
        @decimal, // decimal #
        dot_notation, //dot-notation == hh.hh.hh.hh
        hex, //hex == 0xhh
        Unknown
    }

    [Flags]
    public enum ValueCheckCode
    {
        //normal flags that indicate value is the same that went in or slightly changed
        None = 0,
        LossOfPrecision = 1,
        EquivalentMemberNotPresent = 1 << 1,
        //error flags
        MinimumError = 1 << 2,
        MaximumError = 1 << 3,
        PrecisionError = 1 << 4,
        IndexOutOfRangeError = 1 << 5,
        ParseError = 1 << 6,
        NonViewableType = 1 << 7,
        InvalidCastError = 1 << 8,
        NotInBuild = 1 << 9,
        NoVariableFound = 1<<10,
        AddressOutOfRange = 1<<11
    };

    public class GenBaseValue : GenType, IGenDynamicSize
    {
        internal DisplayFormat _displayFormat;
        public DisplayFormat DisplayFormat { get { return _displayFormat; } }
        internal ValueEncoding _encoding;
        public ValueEncoding Encoding { get { return _encoding; } }
        public bool IsDynamicSize { get { return _encoding == ValueEncoding.String && _byteSize == -1; } }

        public GenBaseValue(string name, Int64 byteSize, ValueEncoding encoding, DisplayFormat format = DisplayFormat.Default)
        {
            _name = name;
            _byteSize = byteSize;
            _encoding = encoding;
            _displayFormat = format;
        }

        public virtual ValueCheckCode GetString(MemoryManager data, out string value, DisplayFormat displayFormat = DisplayFormat.Unknown) //String is universal
        {

            //If format is not overriden use display format of current value
            if (displayFormat == DisplayFormat.Unknown)
                displayFormat = _displayFormat;

            ValueCheckCode code = ValueCheckCode.None;
            if (displayFormat == DisplayFormat.@decimal || displayFormat == DisplayFormat.Default)
            {
                if (_encoding == ValueEncoding.Signed)
                {
                    Int64 signed;
                    code |= GetSigned(data, out signed);
                    value = signed.ToString(CultureInfo.InvariantCulture);
                }
                else if (_encoding == ValueEncoding.Unsigned)
                {
                    UInt64 unsigned;
                    code |= GetUnsigned(data, out unsigned);
                    value = unsigned.ToString(CultureInfo.InvariantCulture);
                }
                else if (_encoding == ValueEncoding.Floating)
                {
                    double floating;
                    code |= GetFloat(data, out floating);
                    value = floating.ToString(CultureInfo.InvariantCulture);
                }
                else if (_encoding == ValueEncoding.String)
                {
                    value = data.GetString(_byteSize);
                }
                else
                {
                    value = "";
                    code = ValueCheckCode.InvalidCastError;
                }
            }
            else if(displayFormat == DisplayFormat.dot_notation)
            {
                var builder = ObjectFactory.StringBuilders.GetObject();
                builder.Clear();
                foreach(byte byteVal in data.GetBytes(_byteSize, ByteOrder.BigEndian))
                {
                    builder.Append(byteVal);
                    builder.Append('.');
                }
                builder.Length = builder.Length - 1;
                value = builder.ToString();
                ObjectFactory.StringBuilders.ReleaseObject(builder);
            }
            else if (displayFormat == DisplayFormat.hex)
            {
                var builder = ObjectFactory.StringBuilders.GetObject();
                builder.Clear();
                builder.Append("0x");
                foreach (byte byteVal in data.GetBytes(_byteSize, ByteOrder.BigEndian))
                {
                    builder.Append(Utilities.Byte2HexTable[byteVal]);
                }
                value = builder.ToString();
                ObjectFactory.StringBuilders.ReleaseObject(builder);
            }
            else
            {
                value = "";
                code = ValueCheckCode.InvalidCastError;
            }

            return code;
        }

        public virtual string GetString(double value, DisplayFormat displayFormat)
        {
            //If format is not overriden use display format of current value
            if (displayFormat == DisplayFormat.Unknown)
                displayFormat = _displayFormat;

            if (_encoding == ValueEncoding.Signed)
            {
                Int64 signed = (Int64)value;
                if (displayFormat == DisplayFormat.@decimal || displayFormat == DisplayFormat.Default)
                    return signed.ToString(CultureInfo.InvariantCulture);
                else if (displayFormat == DisplayFormat.hex)
                    return "0x" + signed.ToString("X" + (_byteSize << 1).ToString());
            }
            else if (_encoding == ValueEncoding.Unsigned)
            {
                UInt64 unsigned = (UInt64)value;
                if (displayFormat == DisplayFormat.@decimal || displayFormat == DisplayFormat.Default)
                    return unsigned.ToString(CultureInfo.InvariantCulture);
                else if (displayFormat == DisplayFormat.hex)
                    return "0x" + unsigned.ToString("X" + (_byteSize << 1).ToString());
            }
            else if (_encoding == ValueEncoding.Floating)
            {
                return value.ToString(CultureInfo.InvariantCulture);
            }
            else if (_encoding == ValueEncoding.String)
            {
                throw new InvalidCastException("Can't cast string to double");
            }

            return value.ToString(CultureInfo.InvariantCulture);
        }

        public virtual ValueCheckCode GetUnsigned(MemoryManager data, out UInt64 value)
        {
            ValueCheckCode code = ValueCheckCode.None;
            if (_encoding == ValueEncoding.Unsigned)
            {
                value = data.GetUnsigned(_byteSize);
            }
            else if(_encoding == ValueEncoding.Signed)
            {
                Int64 signed = data.GetSigned(_byteSize);
                if(signed < 0) //Clamp to min UInt64
                {
                    value = 0;
                    code = (ValueCheckCode.PrecisionError | ValueCheckCode.MinimumError);
                }
                else value = (UInt64)signed;

            }
            else if (_encoding == ValueEncoding.Floating)
            {
                double floating = data.GetFloat(_byteSize);
                if(floating > UInt64.MaxValue) //clamp to UInt64 range
                {
                    value = UInt64.MaxValue;
                    code = (ValueCheckCode.PrecisionError | ValueCheckCode.MaximumError);
                }
                else if(floating < UInt64.MinValue)
                {
                    value = UInt64.MinValue;
                    code = (ValueCheckCode.PrecisionError | ValueCheckCode.MinimumError);
                }
                else value = (UInt64)floating;
            }
            else
            {
                value = 0;
                code = ValueCheckCode.InvalidCastError;
            }

            return code;
        }

        public virtual ValueCheckCode GetSigned(MemoryManager data, out Int64 value)
        {
            ValueCheckCode code = ValueCheckCode.None;
            if (_encoding == ValueEncoding.Signed)
            {
                value = data.GetSigned(_byteSize);
            }
            else if(_encoding == ValueEncoding.Unsigned)
            {
                UInt64 unsigned = data.GetUnsigned(_byteSize);
                if(unsigned > Int64.MaxValue)
                {
                    value = Int64.MaxValue;
                    code |= (ValueCheckCode.PrecisionError | ValueCheckCode.MaximumError);
                }
                value = (Int64)unsigned;
            }
            else if (_encoding == ValueEncoding.Floating)
            {
                double floating = data.GetFloat(_byteSize);
                if(floating > Int64.MaxValue) //clamp to Int64 range
                {
                    value = Int64.MaxValue;
                    code = (ValueCheckCode.PrecisionError | ValueCheckCode.MaximumError);

                }
                else if(floating < Int64.MinValue)
                {
                    value = Int64.MinValue;
                    code = (ValueCheckCode.PrecisionError | ValueCheckCode.MinimumError);
                }
                value = (Int64)floating;
            }
            else
            {
                value = 0;
                code = ValueCheckCode.InvalidCastError;
            }
            return code;
        }

        public virtual ValueCheckCode GetFloat(MemoryManager data, out double value)
        {
            ValueCheckCode code = ValueCheckCode.None;
            if (_encoding == ValueEncoding.Floating)
            {
                value = data.GetFloat(_byteSize);
            }
            else if (_encoding == ValueEncoding.Unsigned)
            {
                UInt64 unsigned = data.GetUnsigned(_byteSize);
                value = (double)unsigned;
            }
            else if (_encoding == ValueEncoding.Signed)
            {
                Int64 signed = data.GetSigned(_byteSize);
                value = (double)signed;
            }
            else
            {
                value = 0;
                code = ValueCheckCode.InvalidCastError;
            }
            return code;
        }


        protected DisplayFormat GetUnknownDisplayFormat(string value)
        {
            DisplayFormat format;
            if (value.StartsWith("0x"))
                format = DisplayFormat.hex;
            else if (value.StartsWith("$"))
                format = DisplayFormat.hex;
            else if (_displayFormat != DisplayFormat.hex)
                format = _displayFormat;
            else
                format = DisplayFormat.Default;

            return format;
        }

        public virtual ValueCheckCode SetString(string value, MemoryManager data, DisplayFormat format = DisplayFormat.Unknown)
        {
            if (format == DisplayFormat.Unknown)
                format = GetUnknownDisplayFormat(value);

            if (format == DisplayFormat.hex)
            {
                if (value.StartsWith("0x"))
                    value = value.Substring(2);
                else if (value.StartsWith("$"))
                    value = value.Substring(1);
            }

            ValueCheckCode code = ValueCheckCode.None;
            if (format == DisplayFormat.@decimal || format == DisplayFormat.Default)
            {
                if (_encoding == ValueEncoding.Floating)
                {
                    double floating;
                    if (double.TryParse(value, NumberStyles.Float, CultureInfo.InvariantCulture, out floating))
                        code |= SetFloat(floating, data);
                    else code |= ValueCheckCode.ParseError;
                }
                else if (_encoding == ValueEncoding.Signed)
                {
                    Int64 signed; double floating;
                    if (Int64.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out signed))
                        code |= SetSigned(signed, data);
                    else if (double.TryParse(value, NumberStyles.Float, CultureInfo.InvariantCulture, out floating))
                        code |= SetFloat(floating, data); //Try to parse to float and clamp or truncate
                    else code |= ValueCheckCode.ParseError;
                }
                else if (_encoding == ValueEncoding.Unsigned)
                {
                    UInt64 unsigned; double floating;
                    if (UInt64.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out unsigned))
                        code |= SetUnsigned(unsigned, data);
                    else if (double.TryParse(value, NumberStyles.Float, CultureInfo.InvariantCulture, out floating))
                        code |= SetFloat(floating, data); //Try to parse to float and clamp or truncate
                    else code |= ValueCheckCode.ParseError;
                }

                else if (_encoding == ValueEncoding.String)
                {
                    if (value.Length > _byteSize)
                    {
                        value = value.Substring(0, (int)_byteSize);
                        code = ValueCheckCode.LossOfPrecision;
                    }
                    data.SetString(value, _byteSize);
                }
            }
            else if (format == DisplayFormat.dot_notation)
            {
                string[] strValues = value.Split('.');
                if (strValues.Length == _byteSize)
                {
                    byte[] bytes = new byte[strValues.Length];
                    for (int ii = 0; ii < bytes.Length; ii++)
                    {
                        bytes[ii] = byte.Parse(strValues[ii], NumberStyles.Integer, CultureInfo.InvariantCulture);
                    }

                    if (_encoding != ValueEncoding.String)
                        data.SetBytes(bytes, 0, _byteSize, ByteOrder.BigEndian);
                    else
                        data.SetBytes(bytes, 0, _byteSize);
                }
                else code |= ValueCheckCode.ParseError;

            }
            else if (format == DisplayFormat.hex)
            {
                int count = value.Length;
                int byteSize = (count + 1) >> 1;
                int startByte = (int)(_byteSize - byteSize);
                int ii = (count & 0x1);

                if (byteSize <= _byteSize)
                {
                    byte[] bytes = new byte[_byteSize];
                    if (ii == 1) //odd number of nibbles given
                        bytes[startByte++] = (byte)(Utilities.NibbleHex2ValueTable[value[0]]);

                    for (; ii < count; ii += 2)
                    {
                        bytes[startByte++] = (byte)((Utilities.NibbleHex2ValueTable[value[ii]] << 4) |
                                           (Utilities.NibbleHex2ValueTable[value[ii + 1]]));
                    }
                    if (_encoding != ValueEncoding.String)
                        data.SetBytes(bytes, 0, _byteSize, ByteOrder.BigEndian);
                    else
                        data.SetBytes(bytes, 0, _byteSize);
                }
                else code |= ValueCheckCode.ParseError;
            }

            return code;
        }

        //TODO: use ref Range range for address instead of UInt64
        public virtual ValueCheckCode SetUnsigned(UInt64 value, MemoryManager data)
        {
            ValueCheckCode code = ValueCheckCode.None;

            if (_encoding == ValueEncoding.Unsigned)
            {
                UInt64 unsigned = Utilities.Clamp((UInt64)value, (UInt32)_byteSize);

                if (unsigned < value)
                    code |= (ValueCheckCode.PrecisionError | ValueCheckCode.MaximumError);
                else if (unsigned > value)
                    code |= (ValueCheckCode.PrecisionError | ValueCheckCode.MinimumError);

                data.SetUnsigned(unsigned, _byteSize);
            }
            else if (_encoding == ValueEncoding.Signed)
            {
                if(value > Int64.MaxValue) //Clampe to Int64 max value
                {
                    value = Int64.MaxValue;
                    code |= (ValueCheckCode.PrecisionError | ValueCheckCode.MaximumError);
                }
                code |= SetSigned((Int64)value, data);
            }
            else if (_encoding == ValueEncoding.Floating)
            {
                code |= SetFloat((double)value, data);
            }
            else code |= ValueCheckCode.InvalidCastError;

            return code;
        }

        public virtual ValueCheckCode SetSigned(Int64 value, MemoryManager data)
        {
            ValueCheckCode code = ValueCheckCode.None;

            if (_encoding == ValueEncoding.Signed)
            {
                Int64 signed = Utilities.Clamp((Int64)value, (UInt32)_byteSize);

                if (signed < value)
                    code |= (ValueCheckCode.PrecisionError | ValueCheckCode.MaximumError);
                else if (signed > value)
                    code |= (ValueCheckCode.PrecisionError | ValueCheckCode.MinimumError);

                data.SetSigned(signed, _byteSize);
            }
            else if (_encoding == ValueEncoding.Unsigned)
            {
                if(value < 0) //Clamp to UInt64 min value
                {
                    value = 0;
                    code |= (ValueCheckCode.PrecisionError | ValueCheckCode.MinimumError);
                }
                code |= SetUnsigned((UInt64)value, data);
            }
            else if(_encoding == ValueEncoding.Floating)
            {
                code |= SetFloat((double)value, data);
            }
            else code |= ValueCheckCode.InvalidCastError;

            return code;
        }

        public virtual ValueCheckCode SetFloat(double value, MemoryManager data)
        {
            ValueCheckCode code = ValueCheckCode.None;

            if (_encoding == ValueEncoding.Floating)
            {
                double floating;
                if (_byteSize == 4)
                {
                    //Clamp to limits
                    if (value > float.MaxValue)
                    {
                        floating = float.MaxValue;
                        code |= (ValueCheckCode.PrecisionError | ValueCheckCode.MaximumError);
                    }
                    else if (value < float.MinValue)
                    {
                        floating = float.MinValue;
                        code |= (ValueCheckCode.PrecisionError | ValueCheckCode.MinimumError);
                    }
                    else floating = (float)value;
                }
                else floating = value;

                data.SetFloat(floating, _byteSize);
            }
            else if(_encoding == ValueEncoding.Signed)
            {
                Int64 signed;
                if (value > Int64.MaxValue) //Clamp in Int64 range
                {
                    signed = Int64.MaxValue;
                    code |= (ValueCheckCode.PrecisionError | ValueCheckCode.MaximumError);
                }
                else if (value < Int64.MinValue)
                {
                    signed = Int64.MinValue;
                    code |= (ValueCheckCode.PrecisionError | ValueCheckCode.MinimumError);
                }
                else signed = (Int64)value;
                code |= SetSigned((Int64)signed, data);
            }
            else if (_encoding == ValueEncoding.Unsigned)
            {
                UInt64 unsigned;
                if (value > UInt64.MaxValue) //Clamp in UInt64 range
                {
                    unsigned = UInt64.MaxValue;
                    code |= (ValueCheckCode.PrecisionError | ValueCheckCode.MaximumError);
                }
                else if (value < UInt64.MinValue)
                {
                    unsigned = UInt64.MinValue;
                    code |= (ValueCheckCode.PrecisionError | ValueCheckCode.MinimumError);
                }
                else unsigned = (UInt64)value;
                code |= SetUnsigned((UInt64)unsigned, data);
            }
            else code |= ValueCheckCode.InvalidCastError;

            return code;
        }

        public virtual ValueCheckCode SetValue(MemoryManager memory, GenBaseValue other, MemoryManager otherMemory, Func<double,double> valueFilter = null)
        {
            ValueCheckCode code = ValueCheckCode.None;
            if (_encoding == ValueEncoding.String)
            {
                if (other._encoding == ValueEncoding.String)
                {
                    string val;
                    code |= other.GetString(otherMemory, out val);
                    code |= SetString(val, memory);
                }
                else code |= ValueCheckCode.InvalidCastError;
            }
            else if (other._encoding != ValueEncoding.String)
            {
                if (_encoding == ValueEncoding.Floating)
                {
                    double floating;
                    //Cast other type val to a double
                    code |= other.GetFloat(otherMemory, out floating);

                    //Check for value filter
                    if (valueFilter != null)
                        floating = valueFilter(floating);

                    if(!double.IsNaN(floating) && !double.IsInfinity(floating))
                        code |= SetFloat(floating, memory);
                    else code = ValueCheckCode.InvalidCastError;
                }
                else if (_encoding == ValueEncoding.Signed)
                {
                    Int64 signed;
                    //Cast other type val to a signed value
                    code |= other.GetSigned(otherMemory, out signed);
                    //Check for value filter
                    if (valueFilter == null)
                        code |= SetSigned(signed, memory);
                    else
                    {
                        double floating = valueFilter(signed);
                        if (!double.IsNaN(floating) && !double.IsInfinity(floating))
                            code |= SetSigned((Int64)floating, memory);
                        else code = ValueCheckCode.InvalidCastError;
                    }
                }
                else if (_encoding == ValueEncoding.Unsigned)
                {
                    UInt64 unsigned;
                    //Cast other type val to an unsigned value
                    code |= other.GetUnsigned(otherMemory, out unsigned);
                    //Check for value filter
                    if(valueFilter == null)
                        code |= SetUnsigned(unsigned, memory);
                    else
                    {
                        double floating = valueFilter(unsigned);
                        if (!double.IsNaN(floating) && !double.IsInfinity(floating))
                            code |= SetUnsigned((UInt64)floating,memory);
                        else code = ValueCheckCode.InvalidCastError;
                    }
                }
                else code |= ValueCheckCode.InvalidCastError;
            }
            else code |= ValueCheckCode.InvalidCastError;

            return code;
        }

        public virtual bool ValuesEqual(MemoryManager memory, GenBaseValue other, MemoryManager otherMemory)
        {
            bool equal = false;
            ValueCheckCode otherChk, chk;
            if (_encoding == ValueEncoding.String)
            {
                if (other._encoding == ValueEncoding.String)
                {
                    string otherVal, val;
                    otherChk = other.GetString(otherMemory, out otherVal);
                    chk = GetString(memory, out val);
                    equal = string.Equals(otherVal, val);
                }
            }
            else if (other._encoding != ValueEncoding.String)
            {
                if (_encoding == ValueEncoding.Floating)
                {
                    double floating, otherFloating;
                    chk = GetFloat(memory, out floating);

                    //Cast other type val to a double
                    otherChk = other.GetFloat(otherMemory, out otherFloating);

                    if(otherChk == ValueCheckCode.None)
                        equal = floating == otherFloating;
                }
                else if (_encoding == ValueEncoding.Signed)
                {
                    Int64 signed, otherSigned;
                    chk = GetSigned(memory, out signed);

                    //Cast other type val to a signed value
                    otherChk = other.GetSigned(otherMemory, out otherSigned);

                    if (otherChk == ValueCheckCode.None)
                        equal = signed == otherSigned;
                }
                else if (_encoding == ValueEncoding.Unsigned)
                {
                    UInt64 unsigned, otherUnsigned;
                    chk = GetUnsigned(memory, out unsigned);

                    //Cast other type cal to an unsigned value
                    otherChk = other.GetUnsigned(otherMemory, out otherUnsigned);

                    if (otherChk == ValueCheckCode.None)
                        equal = unsigned == otherUnsigned;
                }
            }
            return equal;
        }

        public override string ToString()
        {
            return base.ToString();
        }

        public override bool IsImplicitTo(GenType other)
        {
            bool equivalent = true;
            if (other is GenBaseValue)
            {
                var otherVal = other as GenBaseValue;
                if (otherVal._encoding != _encoding)
                {
                    //All non string numeric types are implicit with one another (no checking just clamping, may put in some switches for more behavior)
                    equivalent = otherVal._encoding != ValueEncoding.String && _encoding != ValueEncoding.String;
                }

            }
            else equivalent = false;
            return equivalent;
        }
    }
}
