# gfx-chain

This crates provides means for the users of low-level graphics API of `gfx-hal` to reason about what passes uses what resources and how those resources are accessed.
With this information `gfx-chain` may automatically derive synchronization commands required
before and after each pass.

## Overview

### Resources

Entry point to work with `gfx-chain` is `Resource` trait.
User should implement if for resource types. It is expected to be wrappers for `gfx_hal::Backend::Buffer` and `gfx_hal::Backend::Image`.

### Passes

Passes are logically coupled operations performed by GPU.
For graphics they are identical to Vulkan's subpass.
Pass uses resources the same way each time it is invoked.
Each pass can declare a set of resource-categories it uses. Pass can provide information about how resources from each category is used, how they are accessed and in what layout resources are expected to be.
Additionally pass specify on which command queue operations are executed.

### Chain

Access, layout, usage and queue info (links) collected from all passes associated with each resource-category form dependency chain. Chain automatically derive synchronization commands (pipeline barriers and semaphores) required for resource-category. During command recording chain can be used to insert required barriers for specified concrete resources from the category. For submission chain can be asked if semaphores must be signaled or waited upon.


## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
