using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Variables;
using GenericUtilitiesLib;

namespace EmbedEmul.Types
{
    public class GenSubroutine : GenType, IGenDynamicSize
    {
        internal List<GenType> _returnTypes;
        internal List<GenType> _inputTypes;
        internal List<Variable> _localVars;

        public UInt64 LowPC { get { return _lowPC; } }
        internal UInt64 _lowPC;
        public UInt64 HighPC { get { return _highPC; } }
        internal UInt64 _highPC;

        public bool IsDynamicSize { get { return _isDynamicSize; } }
        internal bool _isDynamicSize;

        /// <summary>
        /// Returns subroutine symbol name
        /// </summary>
        public override string Name { get { return _name; } }
        /// <summary>
        /// Returns input arg type list as string
        /// </summary>
        public string Inputs
        {
            get
            {
                string inputString;
                if (_inputTypes.Count > 0)
                {
                    var build = ObjectFactory.StringBuilders.GetObject();
                    build.Clear();
                    foreach (GenType type in _inputTypes)
                    {
                        build.Append(type.Name);
                        build.Append(", ");
                    }
                    build.Length = build.Length - 2;
                    inputString = build.ToString();
                    ObjectFactory.StringBuilders.ReleaseObject(build);
                }
                else inputString = "void";

                return inputString;
            }
        }

        /// <summary>
        /// Returns output types as string
        /// </summary>
        public string Outputs
        {
            get
            {
                string outputString;
                if (_returnTypes.Count > 0 && _returnTypes[0] != null)
                    outputString = _returnTypes[0].Name;
                else
                    outputString = "void";
                return outputString;
            }
        }

        /// <summary>
        /// Returns function prototype including return and arg types
        /// </summary>
        public override string FullName
        {
            get
            {
                return string.Format("{0} {1}({2})", Outputs, Name, Inputs);
            }
        }

        /// <summary>
        /// Set Program Counter range for current subroutine.
        /// </summary>
        /// <param name="lowPC"></param>
        /// <param name="highPC"></param>
        public void SetPC(UInt64 lowPC, UInt64 highPC)
        {
            _lowPC = lowPC;
            _highPC = highPC;
            _byteSize = (long)(_highPC + 1 - _lowPC);
            _isDynamicSize = false;
        }

        public GenSubroutine(string name)
        {
            if (string.IsNullOrEmpty(name))
                _name = "anonsub_" + _id + "_t";
            else
                _name = name;
            _returnTypes = new List<GenType>();
            _inputTypes = new List<GenType>();
            _localVars = new List<Variable>();
        }

        public override bool IsImplicitTo(GenType other)
        {
            if (other is GenSubroutine)
            {
                var otherSub = other as GenSubroutine;
                if (otherSub._returnTypes.Count == _returnTypes.Count)
                {
                    for (int ii = 0; ii < _returnTypes.Count; ii++)
                    {
                        if (!_returnTypes[ii].IsExplicitTo(otherSub._returnTypes[ii]))
                            return false;
                    }
                }
                else return false;

                if (otherSub._inputTypes.Count == _inputTypes.Count)
                {
                    for (int ii = 0; ii < _inputTypes.Count; ii++)
                    {
                        if (!_inputTypes[ii].IsExplicitTo(otherSub._inputTypes[ii]))
                            return false;
                    }
                }
                else return false;

                return true;
            }
            else return false;
        }

        public override void AppendString(StringBuilder builder)
        {
            builder.AppendLine(FullName);
            builder.AppendLine("{");
            builder.AppendLine("~~Returns~~");
            foreach (GenType type in _returnTypes)
            {
                if (type != null)
                    type.AppendString(builder);
                else
                    builder.AppendLine("void");
            }

            if (_inputTypes.Count > 0)
            {
                builder.AppendLine("~~Arguments~~");
                int idx = 0;
                foreach (GenType type in _inputTypes)
                {
                    builder.Append("[");
                    builder.Append(idx);
                    builder.Append("] ");
                    type.AppendString(builder);
                    idx++;
                }
            }

            if (_localVars.Count > 0)
            {
                builder.AppendLine("~~Local Vars~~");
                foreach (Variable var in _localVars)
                {
                    builder.AppendLine(var.ToString());
                    if (var.Type != null)
                        var.Type.AppendString(builder);
                }
            }

            builder.AppendLine("}");
        }
    }
}
