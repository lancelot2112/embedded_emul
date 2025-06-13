using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Binary;
using EmbedEmul.Memory;

namespace EmbedEmul.Hardware
{
    public enum DeviceType
    {
        Flash,
        EEPROM,
        RAM,
        Cache,
        MMIO,
        Redirect,
        RegisterFile
    }
    public record PrototypeRange
    {
        public string Name;
        public UInt64 Start;
        public UInt64 Size;
        public UInt64 RedirectTo;
        public byte Priority;
        public DeviceType Type;

        public UInt64 ExclusiveEnd { get { return Start + Size; } }

        public PrototypeRange(string name, UInt64 start, UInt64 redirectTo, UInt64 size, DeviceType type, byte priority = 0)
         :this(name, start, size, type, priority)
        {
            RedirectTo = redirectTo;
        }

        public PrototypeRange(string name, UInt64 start, UInt64 size, DeviceType type, byte priority = 0)
        {
            Name = name;
            Start = start;
            Size = size;
            Type = type;
            Priority = priority;
        }
    }

    public struct NamedBusRange
    {
        string _name;
        internal UInt64 _start;
        internal UInt64 _length;
    }

    public static class ModuleDefinition
    {
        /*
        public static IModule Read(string moduleName)
        {
            string defnPath = AppDomain.CurrentDomain.BaseDirectory + "\\Hardware\\" + moduleName;
            if (File.Exists(defnPath))
            {
                using (var fileStream = File.OpenRead(defnPath))
                using (var streamRead = new System.IO.StreamReader(fileStream))
                {

                }

            }

            return null;
        }
        */

        public static void Save(IEnumerable<MemoryUnit> blocks, string moduleName)
        {

        }
    }
}
