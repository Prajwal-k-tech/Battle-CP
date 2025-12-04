# Learning Resources: Rust & Axum

Since you want to learn deeply while we build, here are the best resources to follow along with.

## 1. The Essentials (Start Here)
*   **[Axum Official Docs](https://docs.rs/axum/latest/axum/)**: The holy grail. It's surprisingly readable.
*   **[Tokio Tutorial](https://tokio.rs/tokio/tutorial)**: Axum is built on Tokio. Understanding "Async Rust" (Tasks, Channels, Shared State) is crucial for our game server.
*   **[Rust Book (The Bible)](https://doc.rust-lang.org/book/)**: If you get stuck on ownership or lifetimes, check Chapters 4, 10, and 15.

## 2. Axum Specifics
*   **[Axum 0.7+ Guide (Shuttle.dev)](https://www.shuttle.dev/blog/axum-0-7-guide)**: A fantastic, modern guide on setting up a production-ready Axum app.
*   **[Jeremy Chone's Rust Axum Course (YouTube)](https://www.youtube.com/watch?v=XZtlD_m59sM)**: The best video resource. He explains *why* things are structured the way they are.

## 3. What We Will Learn in Battle CP
As we build, pay attention to these concepts:
1.  **State Management**: How to share the `GameLobby` state across thousands of WebSocket connections using `Arc<RwLock<State>>`.
2.  **Actor Pattern**: We'll use Tokio Tasks to treat each Game Session as an independent "Actor" that processes messages.
3.  **Type-Safe APIs**: How Axum uses Rust's type system to validate requests *before* your handler code even runs.
