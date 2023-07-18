## Deploy MailCrab on fly.io

### 1. Install flyctl

For installation you can follow [this guide](https://fly.io/docs/hands-on/install-flyctl/), or just try the below snippet.

- Linux

```
curl -L https://fly.io/install.sh | sh
```

- MacOs

```
brew install flyctl
```

- Windows

```
pwsh -Command "iwr https://fly.io/install.ps1 -useb | iex"
```

### 2. Login or SignUp to your fly.io account

- If you already have a Fly.io account, you can log in with flyctl by running:

```
fly auth login
```
Your browser will open up with the Fly.io sign-in screen; enter your user name and password to sign in. 

- If you haven’t got a Fly.io account, you’ll need to create one by running:

```
fly auth signup
```
This will open your browser on the sign-up page

### 3. Create a folder with fly.toml config

- Create a folder and a fly.toml config file

```
mkdir mail-crab
cd mail-crab
touch fly.toml
```

- Open fly.toml file with your favorite editor and copy the below config to the file

```toml
[build]
  image = "marlonb/mailcrab:latest"

[[services]]
  protocol = "tcp"
  internal_port = 1080
  processes = ["app"]
  auto_stop_machines = true
  auto_start_machines = true
  min_machines_running = 0

  [[services.ports]]
    force_https = true
    handlers = ["http"]
    port = 80

  [[services.ports]]
    handlers = ["tls", "http"]
    port = 443

  [services.concurrency]
    type = "connections"
    hard_limit = 25
    soft_limit = 20

[[services]]
  protocol = "tcp"
  internal_port = 1025
  auto_stop_machines = true
  auto_start_machines = true
  min_machines_running = 0

  [[services.ports]]
    port = 1025

  [services.concurrency]
    type = "connections"
    hard_limit = 25
    soft_limit = 20
```

### 4. Deploy Mailcrab

- Run the below command in the folder where your fly.toml config file is located, and follow the steps.

```
fly launch --ha=false
```

- Say no to these options

```
? Would you like to set up a Postgresql database now? No
? Would you like to set up an Upstash Redis database now? No
? Would you like to allocate dedicated ipv4 and ipv6 addresses now? No
```

- Allocate shared ipv4 address

```
fly ips allocate-v4 --shared
```

- Allocate ipv6 address

```
fly ips allocate-v6
```

### That's it!

Your SMTP host is your app fly.io domain and the port is 1025
