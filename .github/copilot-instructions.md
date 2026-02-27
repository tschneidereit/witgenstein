## Important process notes

- when a requirement seems inconsistent and it's not fairly obvious how to fix that, ask for clarification instead of making assumptions.
- start writing code sooner rather than later, and iterate on it. Don't wait until you have a perfect design before writing any code.
- write clear, well-documented code, that is easy to understand and maintain. Live up to Alex Crichton's standards.
- reduce allocations as much as is sensible by reusing existing data structures and avoiding unnecessary copies. Live up to Alex Crichton's standards.
- ensure that all added dependencies are at the latest viable version.
- create integration tests, not just unit tests, where applicable.
- do a pass over the code to ensure that there isn't any superfluous code duplication or other warts before declaring a task done.
- add a test for any new functionality, and ensure that all tests pass before declaring a task done.
- ensure that all code is formatted with `cargo fmt` and linted with `cargo clippy` before declaring a task done.
- when receiving specification clarifications, immediately update the specification document to reflect the new understanding, and ensure that the specification is always up to date with the latest understanding of the requirements.
- when receiving new requirements, immediately update the specification document to reflect the new requirements, and ensure that the specification is always up to date with the latest requirements.
- keep a log of all tasks across agent sessions in a TASKLOG.md file, with a brief description of the task, the date it was completed, and a link to the relevant code changes.
- add tasks as `- [ ]` items to the TASKLOG.md file immediately when identifying them, and mark them as done with `- [x]` when completed.
- update tasks immediately if their scope or requirements change, and ensure that the TASKLOG.md file always reflects the current state of tasks and their requirements.
- When using dates in the task log, always use real ones, instead of hallucinated ones, and ensure that they are accurate to the day when the task was completed.
- when working on changes that have UX implications (e.g. changes to CLI output), ensure that the changes are reflected in the specification document, that there are tests covering the new UX behavior, and that the README.md is updated to reflect the new UX behavior as well.

## Project Conventions

- ensure that there is a README.md with a clear description of the project, its purpose, and how to use it. It should first focus on the user-facing aspects of the project, and then either contain details on more technical aspects further down, or link to the specification. The README should be updated immediately when new information is received or new decisions are made, and should always reflect the latest understanding of the project.
- ensure that there is a SPECIFICATION.md that contains all design decisions, implementation details, and any other relevant information about the project. The specification should be updated immediately when new information is received or new decisions are made, and should always reflect the latest understanding of the project.

## Code Conventions

- "Apache-2.0-WITH-LLVM-exception" SPDX identifier for all files
- `unsafe` blocks are documented with safety comments
- for WASIp3 async: nothing is ever allowed to block — use async APIs everywhere, no `block_on` or similar
- never use `wasi:io`, always use WASIp3 async streams and futures instead
