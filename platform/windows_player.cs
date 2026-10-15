using System;
using System.Media;
class SingCoachPlayer {
    static void Main() {
        SoundPlayer player = null;
        try {
            string line;
            while ((line = Console.ReadLine()) != null) {
                string[] args = line.Split('\t');
                try {
                    if (args[0] == "STOP") { if (player != null) player.Stop(); }
                    if ((args[0] == "PLAY" || args[0] == "ONCE") && args.Length == 3) {
                        if (player != null) { player.Stop(); player.Dispose(); }
                        player = new SoundPlayer(args[2]); player.Load(); if (args[0] == "ONCE") player.Play(); else player.PlayLooping();
                        Console.WriteLine("READY\t" + args[1]); Console.Out.Flush();
                    }
                } catch (Exception e) { Console.WriteLine("ERROR\t" + (args.Length > 1 ? args[1] : "0") + "\t" + e.Message.Replace('\n',' ')); Console.Out.Flush(); }
            }
        } finally { if (player != null) {player.Stop();player.Dispose();} }
    }
}
