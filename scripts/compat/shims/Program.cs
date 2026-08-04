// Exercises clients/csharp/Ttid.cs against the binary in TTID_BIN.
using System;
using System.Text.Json;
using Ttid;

internal static class Program
{
    private const string Fixed = "4SQ1NZT5HC0";
    private const string Updated = "4SQ1NZT5HC0-4SQ1NZT5P1S";
    private const string Deleted = "4SQ1NZT5HC0-4SQ1NZT5P1S-4SQ1NZT5WRK";

    private static void Main()
    {
        using var t = new Ttid.Ttid(Environment.GetEnvironmentVariable("TTID_BIN"));
        Console.WriteLine("generate=" + t.Generate().GetRawText());
        Console.WriteLine("update=" + t.Generate(Fixed).GetRawText());
        Console.WriteLine("delete=" + t.Generate(Updated, true).GetRawText());
        Console.WriteLine("decode=" + t.DecodeTime(Deleted).GetRawText());
        Console.WriteLine("isTTID=" + t.IsTTID(Fixed).GetRawText());
        Console.WriteLine("isTTID-bad=" + t.IsTTID("nope").GetRawText());
        Console.WriteLine("isUUID=" + t.IsUUID("3f2504e0-4f89-41d3-9a0c-0305e82c3301").GetRawText());
        Console.WriteLine("isUUID-bad=" + t.IsUUID("nope").GetRawText());
        Console.WriteLine("canonical=" + t.Canonicalize(Fixed.ToLowerInvariant()).GetRawText());
        try
        {
            t.Generate(Deleted);
            Console.WriteLine("error=NO ERROR RAISED");
        }
        catch (TtidException e)
        {
            Console.WriteLine("error=" + e.Message);
        }
    }
}
