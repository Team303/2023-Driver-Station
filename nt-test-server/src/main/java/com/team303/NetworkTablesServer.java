package com.team303;

import edu.wpi.first.networktables.NetworkTable;
import edu.wpi.first.networktables.NetworkTableEntry;
import edu.wpi.first.networktables.NetworkTableEvent;
import edu.wpi.first.networktables.NetworkTableInstance;

import java.io.File;
import java.io.FileWriter;
import java.io.IOException;
import java.util.EnumSet;

public class NetworkTablesServer {
    public static void main(String[] args) throws IOException {
        File store = new File("networktables.json");

        if (!store.exists()) {
            FileWriter fw = new FileWriter(store);
            fw.write("[]");
            fw.close();
        }

        new NetworkTablesServer().run();
    }

    public void run() {
        NetworkTableInstance inst = NetworkTableInstance.getDefault();
        NetworkTable table = inst.getTable("datatable");
        NetworkTableEntry xEntry = table.getEntry("x");
        NetworkTableEntry yEntry = table.getEntry("y");
        NetworkTableEntry testEntry = table.getEntry("test");
        inst.startServer();

        xEntry.setDefaultNumber(5);
        yEntry.setDefaultNumber(25);

        testEntry.setNumber(10);
        testEntry.setPersistent();

        NetworkTable smartDashboard = inst.getTable("SmartDashboard");
        NetworkTableEntry sdCompressorEntry = smartDashboard.getEntry("Compressor Enabled");

        sdCompressorEntry.setBoolean(true);

        //add an entry listener for changed values of "Compressor Enabled"
        smartDashboard.addListener("Compressor Enabled", EnumSet.of(NetworkTableEvent.Kind.kValueAll), (ntTable, ntKey, ntEvent) -> {
            System.out.println("`Compressor Enabled` changed value: " + ntEvent.valueData.value.getBoolean());
        });


        new Thread(() -> {
            while (true) {
                try {
                    Thread.sleep(5000);
                } catch (InterruptedException ex) {
                    System.out.println("interrupted");
                    return;
                }
                double x = xEntry.getDouble(0.0);
                double y = yEntry.getDouble(0.0);
                double test = testEntry.getDouble(0.0);
                System.out.println("X: " + x + " Y: " + y + " Test: " + test);

                boolean compressor = sdCompressorEntry.getBoolean(false);
                System.out.println("Compressor: " + compressor);
            }
        }).start();
    }
}