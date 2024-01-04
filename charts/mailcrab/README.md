# mailcrab

![Version: 0.1.0](https://img.shields.io/badge/Version-0.1.0-informational?style=flat-square) ![Type: application](https://img.shields.io/badge/Type-application-informational?style=flat-square) ![AppVersion: 0.23.0](https://img.shields.io/badge/AppVersion-0.23.0-informational?style=flat-square)

A Helm chart for deploying MailCrab in Kubernetes.

## Values

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| fullnameOverride | string | `""` | Configure the fullname override for resources. |
| image.pullPolicy | string | `"Always"` | Specify an imagePullPolicy, defaults to 'Always' if image tag is 'latest', else set to 'IfNotPresent' |
| image.repository | string | `"marlonb/mailcrab"` | Image to use for the deployment. |
| image.tag | string | `"latest"` | Overrides the image tag whose default is the chart appVersion. |
| imagePullSecrets | list | `[]` | If needed, specity a custom imagePullSecrets to use with priavet registries. |
| ingress.annotations | object | `{}` | Annotations to add to the ingress |
| ingress.className | string | `""` | The class of the Ingress controller to use, default to nginx (nginx, traefik, haproxy) |
| ingress.enabled | bool | `false` | Enables the use of an ingress controller. |
| ingress.hosts[0].host | string | `"chart-example.local"` | The host to use for the ingress. |
| ingress.hosts[0].paths[0].path | string | `"/"` | The path to use for the ingress. |
| ingress.hosts[0].paths[0].pathType | string | `"ImplementationSpecific"` |  |
| ingress.tls | list | `[]` | TLS configuration for ingress |
| nameOverride | string | `""` | Configure the name override for resources. |
| podAnnotations | object | `{}` | Configure annotations to be required by the pods to run the application. |
| podSecurityContext | object | `{}` | Configure the pod security context. |
| replicaCount | int | `1` | Configure the number of replicas to run. |
| resources | object | `{}` | Enable autoscaling for the deployment. |
| securityContext | object | `{}` | Configure the security context for the container. |
| service.containerPort | int | `1080` | The container port to expose on the service. |
| service.port | int | `80` | The port to expose on the service. |
| service.smtpPort | int | `1025` | The container port to expose on the service for the SMTP server. |
| service.type | string | `"ClusterIP"` | The type of service to create. |
| serviceAccount.annotations | object | `{}` | Annotations to add to the service account |
| serviceAccount.create | bool | `true` | Specifies whether a service account should be created |
| serviceAccount.name | string | `""` | The name of the service account to use. If not set and create is true, a name is generated using the fullname template |

