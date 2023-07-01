# Network Tests Readme

Welcome to the Network-Tests project!

## Introduction

The aim of this project is to support the main network tests used by the industry to monitor network quality and service assurance. At the moment, the project only includes the implementation of the TWAMP (Two-Way Active Measurement Protocol) test as per RFC 5357, but it is still a work in progress. The plan is to have a FULL TWAMP implementation along with updated functionalities from RFCs 5618, 5938, 7717, and 7750. Additionally, the project will support other tests such as Ping, Traceroute, DNS Client, STAMP, and TCP Throughput (RFC 6349) in the future.

## Implementation

The project is implemented in Rust programming language. The implementation uses an event loop based on the mio crate.

One of the project's goals is to depend on as few crates as possible to reduce package size and dependencies. This will be done as the implementation evolves.

## Usage

The project is divided into workspace projects, and all tests can be run using the command line with a configuration JSON file as input.

To run the TWAMP test, use the following command:

```bash
cargo run --release --bin twamp -- -c "path_to_configuration_file"
```

### Configuration

Creating a TWAMP sender can be done with the following configuration:

```json
{
  "hosts": [
    {
      "ip": "127.0.0.1",
      "port": 45571
    }
  ],
  "mode": "LIGHT_SENDER",
  "source_ip_address": "0.0.0.0:45572",
  "collection_period": 10,
  "packet_interval": 500,
  "padding": 41,
  "ref_wait": 1,
  "last_message_timeout": 1
}
```

You'll also need a reflector:

```json
{
  "mode": "LIGHT_REFLECTOR",
  "source_ip_address": "0.0.0.0:45571",
  "ref_wait": 2
}
```

## Future Development

The Network-Tests project is still in progress, and there is a plan to support other network tests as mentioned above. The project will be updated regularly, and new features will be added to improve the network quality and service assurance tests.

Thank you for using the Network-Tests project.

## Disclaimer

Please note that the Network-Tests project is a work in progress, and the TWAMP test implementation is currently in the LIGHT mode with no control session. We are continually working on improving the project, and your contributions to the project would be greatly appreciated.

Additionally, we cannot guarantee the accuracy or completeness of the results obtained from the tests. Please use the results at your own discretion.

Thank you for your interest in the Network-Tests project, and we welcome any feedback or suggestions you may have.
