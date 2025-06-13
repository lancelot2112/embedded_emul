using System;
using System.Collections.Generic;
using System.Diagnostics;
using EmbedEmul.Types;
using EmbedEmul.Hardware;

namespace EmbedEmul.SystemBus
{
    #region INTERFACE CONTRACT
    public interface INameBus : IAddressBus
    {
        UInt64 GetValue(string name);
        void SetValue(string name, UInt64 value);
    }

    #endregion

    #region IMPLEMENTATION
    public class RegisterBus : BasicBusAccess, INameBus
    {
        IRegisterTable _table;
        ResolvedRegister _resolved;

        public RegisterBus(IDeviceBus bus, IRegisterTable table)
           : base(bus)
        {
            _table = table;
            _resolved = new ResolvedRegister();
        }


        private void JumpToName(string name)
        {
            if (!_table.ResolveName(name, ref _resolved))
                throw new ArgumentException($"Register '{name}' not found.");

            if (!Jump(_resolved.Instance.Offset))
                throw new ArgumentException($"Cannot jump to register '{name}' at offset {_resolved.Instance.Offset:X}");
        }

        public UInt64 GetValue(string name)
        {
            JumpToName(name);

            var busValue = _range.Device.GetUInt64(_deviceOffset);
            return _resolved.Slice.ReadFrom(busValue);
        }

        public void SetValue(string name, UInt64 value)
        {
            JumpToName(name);

            var busValue = _range.Device.GetUInt64(_deviceOffset);
            _range.Device.SetUInt64(_deviceOffset, _resolved.Slice.WriteOver(busValue, value));
        }


    }
    #endregion
}