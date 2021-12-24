# -

This is a fun project to help me better understand load balancers/proxies. The aim is to implement an
application similar to HAProxy/Envoy.

## Setup

Example config:
```
ip_addr = "127.0.0.1"

ports = [
    8080,
    8081,
    8082,
]

debug = false

[[apps]]
uuid = "test-app"
# This will be used as the anycast address
ip_addr = "127.0.1.0"

[[apps.targets]]
ip_addr = "127.0.1.1"
weight = 100

[[apps.targets]]
ip_addr = "127.0.1.2"
weight = 85

[[apps.targets]]
ip_addr = "127.0.1.3"
weight = 75

[[apps]]
uuid = "test-app2"
# This will be used as the anycast address
ip_addr = "127.0.2.0"

[[apps.targets]]
ip_addr = "127.0.2.1"
weight = 100

[[apps.targets]]
ip_addr = "127.0.2.2"
weight = 85

[[apps.targets]]
ip_addr = "127.0.2.3"
weight = 75
```
