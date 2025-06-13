using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Runtime;
using EmbedEmul.Memory;
using EmbedEmul.SystemBus;

namespace EmbedEmul.Hardware
{
    public abstract class Processor
    {

        internal IDeviceBus ProcessorBus;

        internal virtual void ConnectToBus(IDeviceBus bus)
        {
            foreach (var proto in Prototype)
                bus.RegisterDevice(new BasicMemory(proto.Name, proto.Size), proto.Start);
        }


        public static List<PrototypeRange> Prototype;
        public static UInt16 PercentMatch(List<AddressRange> ranges)
        {
            int protoIdx = 0;
            UInt64 includedBytes = 0;
            UInt64 totalBytes = 1;

            foreach (var range in ranges)
            {
                while (protoIdx < Prototype.Count && range.ExclusiveEnd > Prototype[protoIdx].ExclusiveEnd)
                    protoIdx++;

                if (range.Start >= Prototype[protoIdx].Start)
                {
                    includedBytes += (UInt64)range.Length;
                }

                totalBytes += (UInt64)range.Length;
            }
            return (UInt16)((includedBytes << 8) / totalBytes);
        }
    }

    public static class MachineFactory
    {
        public static List<Type> ProcTypes;
        static MachineFactory()
        {
            ProcTypes = Assembly.GetExecutingAssembly().GetTypes().Where(t => t.IsClass &&
                typeof(Processor).IsAssignableFrom(t)).ToList();
        }
        public static bool BestMatch(List<AddressRange> ranges, out Processor outProc)
        {
            Type bestMatch = null;
            UInt16 bestPercent = 0;
            object[] args = new object[] { ranges };

            foreach (var proc in ProcTypes)
            {
                var percent = (UInt16)proc.InvokeMember("PercentMatch", BindingFlags.Static, null, null, args);
                if (percent > bestPercent)
                {
                    bestPercent = percent;
                    bestMatch = proc;
                }
            }


            if (bestPercent < 50) outProc = null;
            else outProc = (Processor)Activator.CreateInstance(bestMatch);

            return outProc == null;
        }
    }
}