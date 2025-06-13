using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Binary;
using EmbedEmul.Memory;
using EmbedEmul.Tools;

namespace EmbedEmul.Types
{
    public class GenFixedValue : GenBaseValue
    {
        internal double _scale;
        internal double _offset;
        internal string _formatString;

        public GenFixedValue(GenBaseValue valueType, double scale, double offset)
            : this(valueType._name + "_fixed", valueType._byteSize, valueType._encoding, scale, offset)
        { }

        public GenFixedValue(string name, long byteSize, ValueEncoding encoding, double scale, double offset)
            : base(name, byteSize, encoding)
        {
            _scale = scale;
            _offset = offset;

            int digits;
            if (_scale >= 1 || _scale == 0)
                digits = 0;
            else
                digits = (int)(-Math.Log(Math.Abs(_scale), 2));

            //string displayFormat;
            if (digits > 0)
            {
                StringBuilder builder = new StringBuilder();
                builder.Append("{0:0.");
                for (int ii = 0; ii < digits; ii++)
                    builder.Append('0');
                builder.Append("}");

                _formatString = builder.ToString();
            }
            else
                _formatString = "{0:0}";
        }

        public override ValueCheckCode GetString(MemoryManager data, out string value, DisplayFormat format = DisplayFormat.Unknown)
        {
            if (format == DisplayFormat.Unknown)
                format = _displayFormat;

            ValueCheckCode code = ValueCheckCode.None;
            if (format == DisplayFormat.Default || format == DisplayFormat.@decimal)
            {
                double floating;
                code |= GetFloat(data, out floating);
                value = string.Format(_formatString, floating);
            }
            else code |= base.GetString(data,  out value, format);

            return code;
        }

        public override ValueCheckCode GetFloat(MemoryManager data, out double value)
        {
            ValueCheckCode code = ValueCheckCode.None;
            value = data.GetFixed(_byteSize, _encoding != ValueEncoding.Unsigned, _scale, _offset);
            return code;
        }

        public override ValueCheckCode SetString(string value, MemoryManager data, DisplayFormat format = DisplayFormat.Unknown)
        {
            if (format == DisplayFormat.Unknown)
                format = GetUnknownDisplayFormat(value);

            ValueCheckCode code = ValueCheckCode.None;
            if (format == DisplayFormat.Default || format == DisplayFormat.@decimal)
            {
                double floating;
                if (double.TryParse(value, out floating))
                    code |= SetFloat(floating, data);
                else code |= ValueCheckCode.ParseError;
            }
            else code |= base.SetString(value, data, format);

            return code;
        }

        public override ValueCheckCode SetFloat(double value, MemoryManager data)
        {
            ValueCheckCode code = ValueCheckCode.None;
            if (_encoding == ValueEncoding.Signed)
            {
                double floating = Utilities.SignedMinMax[_byteSize, 0] * _scale + _offset;
                double otherFloating = Utilities.SignedMinMax[_byteSize, 1] * _scale + _offset;

                if (value < floating)
                {
                    value = floating;
                    code |= (ValueCheckCode.MinimumError | ValueCheckCode.PrecisionError);
                }
                else if (value > otherFloating)
                {
                    value = otherFloating;
                    code |= (ValueCheckCode.MaximumError | ValueCheckCode.PrecisionError);
                }

                data.SetFixed(value, _byteSize, true, _scale, _offset);
            }
            else if (_encoding == ValueEncoding.Unsigned)
            {
                double floating = Utilities.UnsignedMinMax[_byteSize, 0] * _scale + _offset;
                double otherFloating = Utilities.UnsignedMinMax[_byteSize, 1] * _scale + _offset;

                if (value < floating)
                {
                    value = floating;
                    code |= (ValueCheckCode.MinimumError | ValueCheckCode.PrecisionError);
                }
                else if (value > otherFloating)
                {
                    value = otherFloating;
                    code |= (ValueCheckCode.MaximumError | ValueCheckCode.PrecisionError);
                }

                data.SetFixed(value, _byteSize, false, _scale, _offset);
            }
            else code |= ValueCheckCode.InvalidCastError;

            return code;
        }

        public override ValueCheckCode SetValue(MemoryManager data, GenBaseValue other, MemoryManager otherData,  Func<double, double> valueFilter = null)
        {
            ValueCheckCode code = ValueCheckCode.None;
            if (other._encoding != ValueEncoding.String)
            {
                double floating;
                code |= other.GetFloat(otherData, out floating);

                if (valueFilter != null)
                    floating = valueFilter(floating);

                if (!double.IsNaN(floating) && !double.IsInfinity(floating))
                    code |= SetFloat(floating, data);
            }
            else code = ValueCheckCode.InvalidCastError;

            return code;
        }

        public override bool ValuesEqual(MemoryManager data, GenBaseValue other, MemoryManager otherData)
        {
            ValueCheckCode chk, otherChk;
            bool equal = false;
            if (other._encoding != ValueEncoding.String)
            {
                double floating, otherFloating;
                chk = GetFloat(data, out floating);
                otherChk = other.GetFloat(otherData, out otherFloating);

                if (otherChk == ValueCheckCode.None)
                {
                    double diff = floating > otherFloating ? floating - otherFloating : otherFloating - floating;
                    equal = diff < _scale;
                }
            }

            return equal;
        }
    }
}
