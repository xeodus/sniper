# Sniper Bot

![Build Status](https://github.com/xeodus/sniper/workflows/CI/badge.svg)
![Rust](https://img.shields.io/badge/Rust-006845?style=flat&logo=rust&logoColor=white&labelColor=333333)
![License](https://img.shields.io/badge/License-MIT%20-white.svg)

This is an implementation of a trade bot designed for fast-paced environments like cryto exchanges. This is a low-latency and cross-platform bot written from scratch. It is leverages on robust market algorithms and statistical models to take high-frequency trades. The bot takes care of various market factors both in highly volatile and side ways moving markets. The advanced market-making strategies and risk-management protocols are designed to secure sustained growth and minimize market mishaps. The bot is being primarily developed for ```KuCoin``` and ``Binance`` crypto exchange but hope to deliver for other exchanges too. The bot is designed to be ``memory-safe``, ``concurrent`` and ``asynchronous`` in nature, making it suitablefor high-frequency trading applications;

Sniper is still in its ``early stages of development``, and the code is subject to change. 

## Features

- [x] **Advanced Market-Making Algorithms**
- [x] **Risk Management Protocols**
- [x] **KuCoin and Binance API Integration**
- [x] **WebSocket Integration**
- [x] **Concurrency Module**
- [x] **Memory-Safety**
- [x] **Asynchronous Operations**
- [x] **Unit Tests**

## Pending Work

- [] More efficient error handling
- [] Seemless and blazing-fast WebSocket Integration
- [] Model deployment

## Setup Guide

- **Requirements:** 
- [x] ``Rust 1.70+``
- [x] ``API key`` and ``Secret key`` from exchanges like ``Binance`` or ``KuCoin``

- Keep all of your critical credentials stored inside ``.env`` file for the time being and ``.gitignore`` it.

```bash
# .env

API_KEY="Your_KuCoin_API_key"
SECRET_KEY="Your_KuCoin_secret_key"

API_KEY="Your_Binance_API_key"
SECRET_KEY="Your_Binance_secret_key"

```

- Ensure you have Rust installed. If not, install it from [rustup.rs](https://rustup.rs)

Project setup:

```bash
    git clone https://github.com/xeodus/Sniper.git
    cd Sniper
```
To run unit tests:

```bash
    # Write your own tests
    cd src/Tests

    cargo test
```

Build it:

```bash
    cargo build --release
    cargo run
```

Cheers 🍻

Project is still ``under-development``, everything is still in its trial phase..
Hope to deploy soon 🤞
