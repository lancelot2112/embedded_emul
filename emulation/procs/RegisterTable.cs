 using System;
using System.Collections.Generic;
using EmbedEmul.Types;

namespace EmbedEmul.Hardware
{
    #region INTERFACE CONTRACT
    public interface IRegisterTable
    {
        bool Register(RegisterFile file);
        bool Alias(RegisterFile file, string name);
        bool ResolveName(string name, ref ResolvedRegister res);
    }
    #endregion

    public record RegisterField
    (
        string Label,
        string Description,
        UInt64 Reset,
        BitSlice Slice
    );

    public record RegisterInstance
    (
        string Name,
        UInt64 Offset,
        RegisterFile File
    );
    public class RegisterFile
    {
        public RegisterField Base;
        //Offset is 64 bit aligned
        public UInt64 Offset;
        public UInt16 Count;
        public string NameFormat;
        public Dictionary<string, RegisterField> Fields;
    }

    public class ResolvedRegister
    {
        public RegisterInstance Instance;
        internal RegisterField Field;
        internal string Name;
        internal BitSlice Slice;
    }

    public class RegisterTable : IRegisterTable
    {
        Dictionary<string, RegisterFile> RegisterClass = new Dictionary<string, RegisterFile>();
        Dictionary<string, RegisterInstance> RegisterInstance = new Dictionary<string, RegisterInstance>();

        public bool Register(RegisterFile file)
        {
            if (RegisterClass.TryGetValue(file.Base.Label, out var regFile))
                return false;

            RegisterClass.Add(file.Base.Label, file);
            if (file.Count > 1)
            {
                if (string.IsNullOrEmpty(file.NameFormat))
                    file.NameFormat = $"{file.Base.Label}%d";

                for (int ii = 0; ii < file.Count; ii++)
                {
                    string name = string.Format(file.NameFormat, ii);
                    RegisterInstance.Add(name, new(name, file.Offset + (ulong)(ii << 6), file));
                }
            }
            else
            {
                RegisterInstance.Add(file.Base.Label, new(file.Base.Label, file.Offset, file));
            }
            return true;
        }

        public bool ResolveName(string name, ref ResolvedRegister res)
        {
            if (name == res.Name) return true;

            string[] splitName = name.Split('.');
            if (!RegisterInstance.TryGetValue(splitName[0], out res.Instance))
                throw new ArgumentException($"Register '{name}' not found.");



            if (splitName.Length > 1)
            {
                //Trying to access a field
                if (!res.Instance.File.Fields.TryGetValue(splitName[1], out res.Field))
                    throw new ArgumentException($"Field '{splitName[1]}' not found in register '{name}'.");
            }
            else res.Field = res.Instance.File.Base;

            res.Slice = res.Field.Slice;
            res.Name = name;

            return true;
        }
        public bool Alias(RegisterFile file, string name)
        {
            if (file.Count != 1)
                throw new NotImplementedException();

            var res = new ResolvedRegister();
            ResolveName(name, ref res);

            var existingField = file.Base;
            //Update the alias slice and offset
            file.Base = new RegisterField(
                existingField.Label,
                existingField.Description,
                existingField.Reset,
                res.Slice
            );
            file.Offset = res.Instance.Offset;

            var newInst = new RegisterInstance(file.Base.Label, 0, file);
            if (RegisterInstance.ContainsKey(file.Base.Label))
                RegisterInstance[file.Base.Label] = newInst;
            else RegisterInstance.Add(file.Base.Label, newInst);

            return true;
        }
    }
}