using EmbedEmul.Variables;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EmbedEmul.Types
{
    public class GenClass : GenStructure
    {
        internal List<long> _staticMemberIds;
        internal List<string> _staticMemberLabels;

        public GenClass(string name, UInt32 byteSize)
            : base(name)
        {
            _byteSize = byteSize;
            _name = name;

            _staticMemberIds = new List<long>();
            _staticMemberLabels = new List<string>();
        }

        public void AddStaticMemberLabel(string label)
        {
            _staticMemberLabels.Add(label);
        }
        public void AddStaticMemberId(long id)
        {
            _staticMemberIds.Add(id);
        }

        public IEnumerable<Variable> EnumerateStaticMembers(VariableTable parent)
        {
            Variable memberVariable;
            foreach(long memberId in _staticMemberIds)
            {
                if (parent.TryGetVariable(memberId, out memberVariable))
                    yield return memberVariable;
            }

            foreach(string memberLabel in _staticMemberLabels)
            {
                if (parent.TryGetVariable(memberLabel, out memberVariable))
                    yield return memberVariable;
            }
        }
    }
}
