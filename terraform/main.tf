# Production deployment skeleton for AWS ap-northeast-1 (Tokyo).
#
# This is a skeleton committed at M0 and fleshed out at M13. It intentionally
# documents the intended topology rather than provisioning anything yet, so a
# `terraform apply` here is a no-op. See docs/runbook.md for the deployment
# procedure once this module is complete.
#
# Design notes (why Tokyo):
#   Binance spot infrastructure is widely reported to run in AWS ap-northeast-1.
#   Placing the engine in the same region with warm keep-alive TLS minimises
#   round-trip time to Binance's PUBLIC API gateway. Note: there is no colo
#   cross-connect or shared placement group with Binance's matching engine for
#   API users; a `cluster` placement group only groups instances within THIS
#   account/VPC. The sub-25ms decision-to-ack target is therefore an empirical
#   goal to be measured, not a guarantee, and is gated by Binance rate limits.

terraform {
  required_version = ">= 1.6"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

variable "region" {
  type    = string
  default = "ap-northeast-1"
}

variable "instance_type" {
  type        = string
  default     = "c7i.2xlarge"
  description = "c7i (Intel) or c7g (Graviton). Enhanced networking (ENA) is on by default for these families."
}

provider "aws" {
  region = var.region
}

# TODO(M13): vpc, subnets, security_groups, placement_group (cluster, intra-VPC
# only), ec2 instance with ENA, EBS volumes, IAM role, CloudWatch alarms,
# chrony via user-data. Left unimplemented on purpose so `apply` is a no-op.
