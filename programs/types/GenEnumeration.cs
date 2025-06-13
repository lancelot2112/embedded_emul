using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Binary;
using EmbedEmul.Tools;
using EmbedEmul.Memory;

namespace EmbedEmul.Types
{
    public class GenEnumeration : GenBaseValue
    {
        internal Dictionary<Int64, List<ElementListEntry>> _elementsByValue;
        internal Dictionary<string,ElementListEntry> _elementsByLabel;

        public GenEnumeration(string name, Int32 byteSize, IEnumerable<ElementListEntry> elements)
            : this(name, byteSize)
        {
            foreach(ElementListEntry element in elements)
                AddEnumeration(element);
        }

        public GenEnumeration(string name, Int32 byteSize)
            : base(name, byteSize, ValueEncoding.Signed)
        {
            _elementsByLabel = new Dictionary<string, ElementListEntry>();
            _elementsByValue = new Dictionary<Int64, List<ElementListEntry>>();
        }

        public override ValueCheckCode GetString(MemoryManager data, out string value, DisplayFormat format = DisplayFormat.Unknown)
        {
            if (format == DisplayFormat.Unknown)
                format = _displayFormat;

            ValueCheckCode code = ValueCheckCode.None;
            if (format == DisplayFormat.Default)
            {
                Int64 numericalValue;
                code |= base.GetSigned(data, out numericalValue);

                List<ElementListEntry> entries;
                if (_elementsByValue.TryGetValue(numericalValue, out entries))
                {
                    value = entries[0]._label;
                }
                else
                    value = numericalValue.ToString(CultureInfo.InvariantCulture);
            }
            else code |= base.GetString(data, out value, format);

            return code;
        }

        public override ValueCheckCode SetString(string value, MemoryManager data, DisplayFormat format = DisplayFormat.Unknown)
        {
            if (format == DisplayFormat.Unknown)
                format = _displayFormat;

            ValueCheckCode code = ValueCheckCode.None;
            if (format == DisplayFormat.Default)
            {
                Int64 numericalValue;
                ElementListEntry entry;
                if (_elementsByLabel.TryGetValue(value, out entry)) //See if name
                    numericalValue = entry._value;
                else if (!Int64.TryParse(value, out numericalValue)) //check if provided number directly
                {
                    numericalValue = 0;
                    code |= ValueCheckCode.ParseError;
                }

                code |= base.SetSigned(numericalValue, data);
            }
            else code |= base.SetString(value, data, format);

            return code;
        }

        public void AddEnumeration(string label, Int64 value, string description = "")
        {
            var entry = new ElementListEntry(label, value);
            AddEnumeration(entry);
        }

        public void AddEnumeration(ElementListEntry entry)
        {
            _elementsByLabel.Add(entry._label, entry);
            if (!_elementsByValue.ContainsKey(entry._value))
                _elementsByValue.Add(entry._value, new List<ElementListEntry>() { entry });
            else
                _elementsByValue[entry._value].Add(entry);
        }

        public override void AppendString(StringBuilder builder)
        {
            builder.Append("enum ");
            builder.AppendLine(_name);
            builder.AppendLine("{");
            foreach (ElementListEntry element in _elementsByLabel.Values)
            {
                builder.AppendLine(string.Format("    {1} = {0}", element._value, element._label));
            }
            builder.AppendLine("};");
        }
    }

    public class ElementListEntry
    {
        internal string _label;
        internal Int64 _value;
        internal string _description;

        public ElementListEntry(string label, Int64 value, string description = "")
        {
            _label = label;
            _value = value;
            _description = description;
        }

        public ElementListEntry()
        { }

        public override string ToString()
        {
            return string.Format("{1}<{0}>", _value, _label);
        }
    }
}
