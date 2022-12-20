# Team 303 Driver Station

This is the repository which holds all the code responsible for running our custom driver station. This includes several arduino projects, rust projects, and build scripts. Along with that, schematics and CAD models will also be included when applicable.

## Sub-projects

### `nt-usb-proxy`

This program runs on the driver station laptop and forwards raw TCP/WS packets to the pi over USB, and visa-versa to send USB packets from the pi over TCP/WS to the NetworkTables server.

### `nt-usb-client`

This is a NetworkTables v4 client implementation that uses USB serial instead of TCP/WS to interface with an upstream TCP/WS proxy.

### `nt-test-server`

This is the code for the NetworkTables server used for local testing.

### `arduino-hid`

This is the code which runs on the Arduino(s) simulate HID devices that can be read by the robot code. _(Might not be necessary if triggers are bound to NetworkTables entries)_

### `lcd-display`

This is the code to drive the lcd display on the operator console