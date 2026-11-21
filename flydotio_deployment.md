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

# web interface
[http_service]
  internal_port = 1080
  force_https = true
  auto_stop_machines = "stop"
  auto_start_machines = true
  min_machines_running = 0

  [http_service.concurrency]
    type = "connections"
    hard_limit = 25
    soft_limit = 20

# SMTP server
[[services]]
  protocol = "tcp"
  internal_port = 1025
  auto_stop_machines = "stop"
  auto_start_machines = true
  min_machines_running = 0

  [[services.ports]]
    port = 1025

  [services.concurrency]
    type = "connections"
    hard_limit = 25
    soft_limit = 20
```

MailCrab keeps all messages in memory, so they are gone once the machine is stopped. Set `auto_stop_machines = "off"`
in both sections if you want to keep the messages around while MailCrab is idle.

### 4. Deploy Mailcrab

- Run the below command in the folder where your fly.toml config file is located, and follow the steps.

```
fly launch --copy-config --ha=false --no-db --no-redis --no-object-storage
```

The app name and region are added to your fly.toml by `fly launch`. MailCrab does not need a database, Redis or
object storage, and `--ha=false` makes sure only a single machine is created (messages are not shared between machines).

- Allocate an IP address for the SMTP server

The web interface is reachable on the shared IPv4 address and the IPv6 address that fly.io assigns on the first deploy.
The SMTP server is a plain TCP service on port 1025 and is not reachable on a shared IPv4 address. Check the
addresses of your app using:

```
fly ips list
```

Make sure there is an IPv6 address (free):

```
fly ips allocate-v6
```

If you need to reach the SMTP server over IPv4, allocate a dedicated IPv4 address (billed monthly):

```
fly ips allocate-v4
```

### That's it!

Your SMTP host is your app fly.io domain and the port is 1025
