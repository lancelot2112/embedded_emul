using System;
using System.Collections.Generic;
using System.Diagnostics;

namespace EmbedEmul.SystemBus.DataBusExtensions
{
    #region STRING EXTENSIONS
    public static class DataBusStringExtensions
    {
        public static string GetString(this IDataBus bus, int maxLen = -1)
        {
            return "";
        }

        public static void SetString(this IDataBus bus, string value, int maxLen = -1)
        {

        }
    }
    #endregion
}