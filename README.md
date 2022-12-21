# Team 303 Driver Station

This is the repository which holds all the code responsible for running our custom driver station. This includes several arduino projects, rust projects, and build scripts. Along with that, schematics and CAD models will also be included when applicable.

## Sub-projects

### `nt-test-server`

This is the code for the NetworkTables server used for local testing.

### `nt-usb-proxy`

This program runs on the driver station laptop and forwards raw TCP/WS packets to the pi over USB, and visa-versa to send USB packets from the pi over TCP/WS to the NetworkTables server.

### `nt-usb-client`

This is a NetworkTables v4 client implementation that uses USB serial instead of TCP/WS to interface with an upstream TCP/WS proxy.

### `nt-usb-proto`

This is a shared library for encoding and decoding messages sent over USB.

### `lcd-display`

This is the code to drive the lcd display on the operator console

## Methods

Multiple methods were considered, but for the most flexibility, I chose method 1.

### Method 1 (Most complex)

DS runs nt proxy, send packets over usb. Pi runs nt usb client, and sends packets back over usb.

Most versatile (can send and receive any arbitrary nt data)

### Method 2 (Less complex, but still difficult)

DS runs nt client, sends data over usb serial. Pi runs usb client that sends custom packets over serial about which buttons are pressed.

DS does all the real nt comms

### Method 3 (Might not work/be reliable)

DS runs nothing. Pi connects to DS over ethernet (pi side) to usb (DS side) adapter, and uses ICS on DS. Pi has direct access to nt over tcp.

No guarantee that FMS will like this, or that it will even work/be allowed.

