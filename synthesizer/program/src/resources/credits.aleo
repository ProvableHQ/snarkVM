// Copyright 2024 Aleo Network Foundation
// This file is part of the snarkVM library.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:

// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

/********************************************** INVARIANTS ************************************************************/

// 1. contains committee[r0] =>
//   a. contains delegated[r0]
//      - The set of delegated addresses is superset of active validators.
// 2. delegated[*] >= 10,000
//   a. contain bonded[*].microcredits >= 10,000
//      - Delegators can bond to any address with the 10,000 credit minimum
// 3. delegated[r0] < 10,000,000 =>
//   a. !contain committee[r0]
//      - If the delegated total (which includes the self bond) is less than 10,000,000 credits, it won't be in the committee
// 4. bond[r0].validator == r0 =>
//   a. contains committee[r0]
//     - Self bonded addresses are always active validators. Delegators **can** force validators to unbond by removing sufficient delegation
// 5. committee[r0] =>
//   a. bonded[r0].microcredits >= 100 credits
//      - Self bonded addresses are always active validators with at least 100 credits self bonded
//   b. delegated[r0] >= 10,000,000 credits
//      - The total delegated amount for an active validator is at least 10,000,000 credits
// 6. delegated[r0] == sum bond[*].microcredits by bond[].validator == r0
// 7. bond[r0].microcredits < 10,000 =>
//   a. bond[r0].validator == r0
//   b. contains committee[r0]
//   - A bond value < 10,000 implies self-bonded active validator.
// 8. bond[r0].validator != r0 =>
//   a. bond[r0].microcredits >= 10,000
// 9. count bonded[] - count committee[] == metadata[aleo1qgqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqanmpl0]
//   - Delegator count consistent with metadata.
// 10. count committee[] == metadata[aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc]
// 11. 100,000 + count committee[] >= count bonded[] >= count delegated[] >= count committee[]
// 12. bond[*].microcredits > 100


/**********************************************************************************************************************/

program credits.aleo;

/**********************************************************************************************************************/

/// The `committee` mapping contains the active validator set and their corresponding stake.
mapping committee:
    // The key represents the address of the validator.
    key as address.public;
    // The value represents the committee state of the validator.
    value as committee_state.public;

// The `committee_state` struct tracks the total stake of the validator, and whether they are open to new stakers.
struct committee_state:
    // The boolean flag indicating if the validator is open to new stakers.
    is_open as boolean;
    // The percentage amount (from 0 to 100, inclusive) of rewards that retained by the validator.
    commission as u8;

/**********************************************************************************************************************/

// The `delegated` mapping tracks the total amount of microcredits that are prebonded and bonded to validator addresses.
// Note: The mapping includes both prebonded and bonded microcredits. However, it does not contain unbonding microcredits.
mapping delegated:
    // The key represents the address of the validator.
    key as address.public;
    // The value represents the amount of microcredits bonded to the validator, by the validator and its delegators.
    value as u64.public;

/**********************************************************************************************************************/

/// The `metadata` mapping stores:
///   - The number of members in the committee.
///   - The number of delegators.
mapping metadata:
    // The key represents the index at which the count is stored.
    //    - This address (aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc) stores the number of **members** in the committee.
    //    - This address (aleo1qgqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqanmpl0) stores the number of **delegators**.
    key as address.public;
    // The value represents the count.
    value as u32.public;

/**********************************************************************************************************************/

// The `bonded` mapping represents the amount of microcredits that are currently bonded.
mapping bonded:
    // The key represents the address of the staker, which includes the validators and their delegators.
    key as address.public;
    // The value represents the bond state.
    value as bond_state.public;

// The `bond_state` struct tracks the amount of microcredits that are currently bonded to the specified validator.
struct bond_state:
    // The address of the validator.
    validator as address;
    // The amount of microcredits that are currently bonded to the specified validator.
    microcredits as u64;

/**********************************************************************************************************************/

// The `unbonding` mapping contains a set of stakers with their unbonding microcredits and unlock height.
mapping unbonding:
    // The key represents the address of the staker, which includes the validators and their delegators.
    key as address.public;
    // The value represents the unbond state.
    value as unbond_state.public;

// The `unbond_state` struct tracks the microcredits that are currently unbonding, along with the unlock height.
struct unbond_state:
    // The amount of microcredits that are currently unbonding.
    microcredits as u64;
    // The block height at which the unbonding will be complete, and can be claimed.
    height as u32;

/**********************************************************************************************************************/

// The `account` mapping is used to store credits publicly.
mapping account:
    // The key represents the address of the owner.
    key as address.public;
    // The value represents the amount of public microcredits that belong to the specified owner.
    value as u64.public;

/**********************************************************************************************************************/

// The `withdraw` mapping contains the staking address and their corresponding withdrawal address.
mapping withdraw:
    // The key represents the staking address of the owner.
    key as address.public;
    // The value represents the withdrawal address of the owner.
    value as address.public;

/**********************************************************************************************************************/

// The `credits` record is used to store credits privately.
record credits:
    // The address of the owner.
    owner as address.private;
    // The amount of private microcredits that belong to the specified owner.
    microcredits as u64.private;

/**********************************************************************************************************************/

// This function allows a validator to bond their microcredits to themselves and specify a withdrawal address
// and commission percentage.
//
// The commission percentage is the portion of rewards that the validator may claim for itself.
// Note: Setting the commission percentage to 100 results in all rewards being retained for the validator.
//
// The corresponding functions for 'bond_validator' are 'unbond_public' and 'claim_unbond_public'.
function bond_validator:
    // Input the withdrawal address.
    input r0 as address.public;
    // Input the amount of microcredits to bond.
    input r1 as u64.public;
    // Input the commission percentage.
    input r2 as u8.public;

    // Ensure the withdrawal address is not the validator address.
    assert.neq self.signer r0;

    // Determine if the amount is at least 1 credit.
    gte r1 1_000_000u64 into r3;
    // Enforce the amount is at least 1 credit.
    assert.eq r3 true;

    // Ensure the commission percentage does not exceed 100%.
    gt r2 100u8 into r4;
    assert.neq r4 true;

    // Bond the specified amount of microcredits to the specified validator.
    async bond_validator self.signer r0 r1 r2 into r5;
    // Output the finalize future.
    output r5 as credits.aleo/bond_validator.future;

finalize bond_validator:
    // Input the validator's address.
    input r0 as address.public;
    // Input the withdrawal address.
    input r1 as address.public;
    // Input the amount of microcredits to bond.
    input r2 as u64.public;
    // Input the commission percentage.
    input r3 as u8.public;

    // Retrieve the withdrawal address for the validator.
    get.or_use withdraw[r0] r1 into r4;
    // Ensure that the withdrawal address is consistent.
    assert.eq r1 r4;

    // Construct the initial committee state.
    // Note: We set the initial 'is_open' state to 'true'.
    cast true r3 into r5 as committee_state;
    // Retrieve the committee state of the specified validator.
    get.or_use committee[r0] r5 into r6;

    // Ensure the commission percentage remains unchanged.
    // Note: The commission percentage can only be set when the validator bonds for the first time.
    assert.eq r3 r6.commission;

    /* Bonded */

    // Construct the initial bond state.
    cast r0 0u64 into r7 as bond_state;
    // Get the bond state for the signer, or default to the initial bond state.
    get.or_use bonded[r0] r7 into r8;
    // Enforce the validator matches in the bond state.
    assert.eq r8.validator r0;

    // Increment the amount of microcredits for the bond state.
    add r8.microcredits r2 into r9;
    // Construct the updated bond state.
    cast r0 r9 into r10 as bond_state;

    /* Delegated */

    // Include total pre-delegated amount when deciding if validator has enough credits to join committee.
    // First get the current delegated amount, or start with 0 if none delegated.
    get.or_use delegated[r0] 0u64 into r11;
    // Self-bond amount + existing delegated amount = total bond.
    add r2 r11 into r12;
    // Determine if the amount is at least 10 million credits.
    gte r12 10_000_000_000_000u64 into r13;
    // Enforce the amount is at least 10 million credits.
    assert.eq r13 true;

    /* Account */

    // Get the balance of the signer.
    // If the account does not exist, this finalize scope will fail.
    get account[r0] into r14;
    // Decrement the balance of the signer.
    sub r14 r2 into r15;

    /* Writes */

    // if (new_validator)
    contains committee[r0] into r16;
    branch.eq r16 true to validator_in_committee;
    // {
        // Set the withdrawal address.
        // Note: This operation is only necessary on the first time for a validator entry, in order to initialize the value.
        set r4 into withdraw[r0];

        // Check if the initial bond amount is at least 100 credits.
        gte r2 100_000_000u64 into r17;
        // Ensure that the initial bond is at least 100 credits.
        assert.eq r17 true;

        // Get the committee size.
        get.or_use metadata[aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc] 0u32 into r18;
        // Increment the committee size by one.
        add r18 1u32 into r19;
        // Set the new committee size.
        set r19 into metadata[aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc];

        // Check if the validator exists in the unbonding state.
        contains unbonding[r0] into r20;
        // Ensure the validator currently is not unbonding.
        assert.eq r20 false;
    // }

    position validator_in_committee;

    // Update the committee state of the specified validator.
    set r6 into committee[r0];
    // Update the delegated amount.
    set r12 into delegated[r0];
    // Update the bond state for the signer.
    set r10 into bonded[r0];
    // Update the balance of the signer.
    set r15 into account[r0];

/**********************************************************************************************************************/

// This function allows any delegator to bond their microcredits to a validator and specify a withdrawal address.
//
// The declared validator is **not** required to be in the committee yet, however it may **not** be unbonding.
// This function enforces a minimum of 10,000 credits for each delegator.
// The maximum number of delegators allowed in the committee is 100,000 delegator addresses.
//
// The corresponding functions for 'bond_public' are 'bond_validator', 'unbond_public', and 'claim_unbond_public'.
function bond_public:
    // Input the validator's address.
    input r0 as address.public;
    // Input the withdrawal address.
    input r1 as address.public;
    // Input the amount of microcredits to bond.
    input r2 as u64.public;

    // Determine if the amount is at least one credit.
    gte r2 1_000_000u64 into r3;
    // Enforce the amount is at least one credit.
    assert.eq r3 true;

    // Do not allow self-bonding. Validators start with bond_validator.
    assert.neq self.caller r0;

    // Bond the specified amount of microcredits to the specified validator.
    async bond_public self.caller r0 r1 r2 into r4;
    // Output the finalize future.
    output r4 as credits.aleo/bond_public.future;

finalize bond_public:
    // Input the staker's address.
    input r0 as address.public;
    // Input the validator's address.
    input r1 as address.public;
    // Input the withdrawal address.
    input r2 as address.public;
    // Input the amount of microcredits to bond.
    input r3 as u64.public;

    // Retrieve the withdrawal address for the staker.
    get.or_use withdraw[r0] r2 into r4;
    // Ensure that the withdrawal address is consistent.
    assert.eq r2 r4;

    // Check if the delegator is already bonded to the validator.
    contains bonded[r0] into r5;
    branch.eq r5 true to continue_bond_delegator;
    // {
        // Set the withdrawal address.
        // Note: This operation is only necessary on the first time for a staker entry, in order to initialize the value.
        set r2 into withdraw[r0];

        // Ensure that the validator is open to new stakers. By default, `is_open` is set to `true`.
        cast true 0u8 into r6 as committee_state;
        get.or_use committee[r1] r6 into r7;
        assert.eq r7.is_open true;

        // Get the number of delegators.
        get.or_use metadata[aleo1qgqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqanmpl0] 0u32 into r8;
        // Increment the number of bonded delegators by one.
        add r8 1u32 into r9;
        // Determine if the number of delegators is less than or equal to 100_000.
        lte r9 100_000u32 into r10;
        // Enforce that the number of delegators is less than or equal to 100_000.
        assert.eq r10 true;
        // Set the new number of delegators.
        set r9 into metadata[aleo1qgqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqanmpl0];
    // }

    position continue_bond_delegator;

    /* Bonded */

    // Construct the initial bond state.
    cast r1 0u64 into r11 as bond_state;
    // Get the bond state for the caller, or default to the initial bond state.
    get.or_use bonded[r0] r11 into r12;
    // Enforce the validator matches in the bond state.
    assert.eq r12.validator r1;

    // Increment the microcredits in the bond state.
    add r12.microcredits r3 into r13;

    // Determine if the amount is at least 10 thousand credits.
    gte r13 10_000_000_000u64 into r14;
    // Enforce the amount is at least 10 thousand credits.
    assert.eq r14 true;

    // Construct the updated bond state.
    cast r1 r13 into r15 as bond_state;

    /* Account */

    // Get the balance of the caller.
    // If the account does not exist, this finalize scope will fail.
    get account[r0] into r16;
    // Decrement the balance of the caller.
    sub r16 r3 into r17;

    /* Delegated */

    // Get current total delegated amount.
    get.or_use delegated[r1] 0u64 into r18;
    // Add new bond amount to current delegation.
    add r3 r18 into r19;

    /* Unbonding */

    // Check if the validator exists in the unbonding state.
    contains unbonding[r1] into r20;
    // Ensure the validator currently is not unbonding.
    assert.eq r20 false;

    /* Writes */

    // Update the bond state for the caller.
    set r15 into bonded[r0];
    // Update the balance of the caller.
    set r17 into account[r0];
    // Update delegated amount.
    set r19 into delegated[r1];

/**********************************************************************************************************************/

// This function allows any staker to unbond their microcredits from a validator,
// callable only by using the withdrawal address of the staker or the validator.
//
// **Validators**: It will remove the validator if the self bonded balance falls below 100 credits
// or the total delegated balance falls below 10M credits.
// **Delegators**: It will remove the validator if the total delegated balance falls below 10M credits.
// It will remove the entire bond_state of the delegator if it falls below 10,000 credits.
//
// Validators are permitted to fully unbond any of their delegators. When a validator unbonds a delegator,
// the entire bonded balance is unbonded, regardless of the amount of microcredits and may end up removing the validator
// from the committee if the total delegated balance falls below 10M credits.
//
// The corresponding function for 'unbond_public' is 'claim_unbond_public'.
function unbond_public:
    // Input the staker's address.
    input r0 as address.public;
    // Input the amount of microcredits to unbond.
    input r1 as u64.public;

    // Unbond the specified amount of microcredits for the specified validator.
    async unbond_public self.caller r0 r1 into r2;
    // Output the finalize future.
    output r2 as credits.aleo/unbond_public.future;

finalize unbond_public:
    // Input the caller's address.
    input r0 as address.public;
    // Input the staker's address.
    input r1 as address.public;
    // Input the amount of microcredits to unbond.
    input r2 as u64.public;

    // Set the default unbonding state.
    add block.height 360u32 into r3;
    // Construct the default unbonding state.
    cast 0u64 r3 into r4 as unbond_state;

    // Retrieve the staker's bond state.
    // Note: If the bonded state does not exist, reject the transition.
    get bonded[r1] into r5;

    /* Check Caller's Permission */

    // Get the staker's withdrawal address.
    get withdraw[r1] into r6;
    // Check if the caller's address equals the staker's withdrawal address.
    is.eq r0 r6 into r7;

    // Check if the validator's withdrawal address has been set.
    // Note: This contains check must use `withdraw` and **not** `committee` to ensure the validator
    // can unbond delegators even when the validator is not in the committee. Of course, if the validator is
    // in the committee, then the validator must be able to unbond its delegators.
    contains withdraw[r5.validator] into r8;
    // Get the validator's withdrawal address from the bond state, using the zero address as the default.
    get.or_use withdraw[r5.validator] aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc into r9;
    // Check if the validator's withdrawal address matches the caller.
    is.eq r0 r9 into r10;
    // AND the withdraw address has been set (to prevent an edge case where a validator uses the zero address as a withdrawal address).
    and r8 r10 into r11;

    // Either the caller is the staker's withdrawal address OR the validator's withdrawal address is set to the caller.
    or r7 r11 into r12;
    // Enforce the permission as `true`.
    assert.eq r12 true;

    // Check if bonded to self (validator) or to a different address (delegator).
    is.eq r5.validator r1 into r13;
    branch.eq r13 true to unbond_validator;

    /* Unbond the Delegator */
    // {
        // Retrieve or initialize the unbonding state.
        get.or_use unbonding[r1] r4 into r14;
        // Retrieve the delegated amount in microcredits for the validator.
        get delegated[r5.validator] into r15;

        // Calculate new bonded microcredits.
        // Note: If the subtraction underflows, reject the transition.
        sub r5.microcredits r2 into r16;

        // Check if the delegator will fall below 10,000 bonded credits.
        lt r16 10_000_000_000u64 into r17;

        // If the validator is forcing the delegator to unbond OR the delegator will fall below 10,000 bonded credits.
        or r11 r17 into r18;

        // Determine the amount to unbond: requested amount if >= 10,000 credits, otherwise the full bonded amount.
        ternary r18 r5.microcredits r2 into r19;

        /* Unbonding */

        // Increment existing unbond amount by the new unbond amount.
        // Note: If the addition overflows, reject the transition.
        add r14.microcredits r19 into r20;
        // Construct the updated unbond state.
        cast r20 r3 into r21 as unbond_state;
        // Store the new unbonding state.
        set r21 into unbonding[r1];

        /* Delegated */

        // Calculate the new delegated total for the validator.
        // Note: If the subtraction underflows, reject the transition.
        sub r15 r19 into r22;
        // Store the new delegated total.
        set r22 into delegated[r5.validator];

        // Check if the new bonded state is falling below 10,000 credits, or if the validator is forcing an unbond on the delegator.
        branch.eq r18 true to remove_delegator;

        /* Decrement the Delegator */
        // {
            /* Bonded */

            // Construct the new bond state.
            cast r5.validator r16 into r23 as bond_state;
            // Set the new bond state.
            set r23 into bonded[r1];

            // Jump to the end of the unbond delegator process.
            branch.eq true true to end_unbond_delegator;
        // }

        position remove_delegator;
        // {
            /* Bonded */

            // Remove the bonded state.
            remove bonded[r1];

            /* Metadata */

            // Retrieve the current number of delegators.
            get metadata[aleo1qgqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqanmpl0] into r24;
            // Decrement the number of delegators by one.
            sub r24 1u32 into r25;
            // Store the new number of delegators.
            set r25 into metadata[aleo1qgqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqanmpl0];
        // } Intentionally fall through to end_unbond_delegator

        position end_unbond_delegator;
        // {
            // Check if the new delegated total is at least 10M credits.
            gte r22 10_000_000_000_000u64 into r26;
            // Jump to end if the delegated total is at least 10M credits.
            branch.eq r26 true to end;
        // }
        // If the delegated total is below 10M credits, continue to unbonding the validator.
    //}

    /* Unbond the Validator */
    position unbond_validator;
    // {
        // Check if the committee contains the validator.
        contains committee[r5.validator] into r27;
        // Determine if the delegator unbonding from a validator is not in the committee.
        nor r13 r27 into r28;
        // Jump to the end, if the delegator unbonding from a validator is not in the committee.
        branch.eq r28 true to end;

        // Retrieve the committee state.
        // Note: If the committee state does not exist, reject the transition.
        get committee[r5.validator] into r29;
        // Retrieve the bond state of the validator.
        // Note: If the bonded state does not exist, reject the transition.
        get bonded[r5.validator] into r30;
        // Retrieve the total delegated balance for the validator.
        // Note: If the delegated state does not exist, reject the transition.
        get delegated[r5.validator] into r31;

        // Check if the current delegated amount is below 10M credits.
        // Note: This will only be `true` if a delegator unbond caused the validator to fall below 10M credits.
        lt r31 10_000_000_000_000u64 into r32;
        branch.eq r32 true to remove_validator;

        // Calculate the updated delegated total after unbonding.
        // Note: If the subtraction underflows, reject the transition.
        sub r31 r2 into r33;

        // Calculate the updated amount of microcredits after unbonding.
        sub r30.microcredits r2 into r34;

        // Check if the new bond state is below the required minimums.
        gte r34 100_000_000u64 into r35; // Minimum self bond requirement: 100 credits.
        gte r33 10_000_000_000_000u64 into r36; // Minimum total delegated requirement: 10M credits.

        // If either bond state is below the thresholds, remove the validator from the committee.
        and r35 r36 into r37;
        branch.eq r37 false to remove_validator;

        /* Unbonding */

        // Retrieve or initialize the unbonding state.
        get.or_use unbonding[r5.validator] r4 into r38;
        // Increment the unbond amount.
        // Note: If the addition overflows, reject the transition.
        add r38.microcredits r2 into r39;
        // Set the updated unbond state.
        cast r39 r3 into r40 as unbond_state;
        // Store the new unbonding state.
        set r40 into unbonding[r5.validator];

        /* Delegated */

        // Store the new delegated total.
        set r33 into delegated[r5.validator];

        /* Bonded */

        // Construct and store the new bond state.
        cast r5.validator r34 into r41 as bond_state;
        set r41 into bonded[r5.validator];

        // Jump to end.
        branch.eq true true to end;
    //}

    /* Remove the Validator from the Committee */
    position remove_validator;
    // {
        /* Committee */

        // Remove the validator from the committee.
        remove committee[r5.validator];

        /* Metadata */

        // Retrieve the current committee size.
        get metadata[aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc] into r42;
        // Decrement the committee size by one.
        sub r42 1u32 into r43;
        // Store the new committee size.
        set r43 into metadata[aleo1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3ljyzc];

        /* Delegated */

        // Decrease the delegated total by the bonded amount.
        sub r31 r30.microcredits into r44;
        // Store the new delegated total.
        set r44 into delegated[r5.validator];

        /* Bonded */

        // Remove the bonded state.
        remove bonded[r5.validator];

        /* Unbonding */

        // Retrieve or initialize the unbonding state.
        get.or_use unbonding[r5.validator] r4 into r45;
        // Increment the unbond amount by the full bonded amount.
        // Note: If the addition overflows, reject the transition.
        add r30.microcredits r45.microcredits into r46;
        // Construct the updated unbond state.
        cast r46 r3 into r47 as unbond_state;
        // Store the new unbonding state.
        set r47 into unbonding[r5.validator];
    //}

    position end;

/**********************************************************************************************************************/

// The `claim_unbond_public` function allows any staker to claim their microcredits
// to their withdrawal address after the unbonding period.
//
// Note: Please be advised that this function is callable by any user for any staker.
//
// This function also removes the staker's withdrawal address if the staker no longer has any bonded balance.
function claim_unbond_public:
    // Input the staker's address.
    input r0 as address.public;

    // Claim the unbonded microcredits.
    async claim_unbond_public r0 into r1;
    // Output the finalize future.
    output r1 as credits.aleo/claim_unbond_public.future;

finalize claim_unbond_public:
    // Input the staker's address.
    input r0 as address.public;

    // Get the unbond state for the address, or fail if it does not exist.
    get unbonding[r0] into r1;
    // Determine if unbonding is complete.
    gte block.height r1.height into r2;
    // Enforce the unbonding is complete.
    assert.eq r2 true;

    /* Withdraw */

    // Get the withdrawal address for the address.
    get withdraw[r0] into r3;

    /* Account */

    // Add the unbonded amount to the withdrawal address public balance.
    // Increments `account[r3]` by `r1.microcredits`.
    // If `account[r3]` does not exist, 0u64 is used.
    // If `account[r3] + r1.microcredits` overflows, `claim_unbond_public` is reverted.
    get.or_use account[r3] 0u64 into r4;
    add r1.microcredits r4 into r5;
    set r5 into account[r3];

    /* Unbonding */

    // Remove the unbond state for the staker.
    remove unbonding[r0];

    // Check if the staker is still bonded.
    contains bonded[r0] into r6;
    // Ends the `claim_unbond_public` logic.
    branch.eq r6 true to end;

    /* Withdraw */

    // If the caller is no longer bonded, remove the withdrawal address.
    remove withdraw[r0];

    // The terminus.
    position end;

/**********************************************************************************************************************/

// This function allows a validator to set their state to be either opened or closed to new stakers.
// When the validator is open to new stakers, any staker (including the validator) can bond or unbond from the validator.
// When the validator is closed to new stakers, existing stakers can still bond or unbond from the validator, but new stakers cannot bond.
//
// This function serves two primary purposes:
// 1. Allow a validator to leave the committee, by closing themselves to stakers and then unbonding all of their stakers.
// 2. Allow a validator to maintain their % of stake, by closing themselves to allowing more stakers to bond to them.
function set_validator_state:
    // Input the 'is_open' state.
    input r0 as boolean.public;
    // Set the validator to be either open or closed to new stakers.
    async set_validator_state self.caller r0 into r1;
    // Output the finalize future.
    output r1 as credits.aleo/set_validator_state.future;

finalize set_validator_state:
    // Input the validator's address.
    input r0 as address.public;
    // Input the 'is_open' state.
    input r1 as boolean.public;

    // Get the committee state for the specified validator.
    // If the validator does not exist, this finalize scope will fail.
    get committee[r0] into r2;

    // Construct the updated committee state.
    cast r1 r2.commission into r3 as committee_state;
    // Update the committee state for the specified validator.
    set r3 into committee[r0];

/**********************************************************************************************************************/

// The `transfer_public` function sends the specified amount
// from the caller's `account` to the receiver's `account`.
function transfer_public:
    // Input the receiver.
    input r0 as address.public;
    // Input the amount.
    input r1 as u64.public;
    // Transfer the credits publicly.
    async transfer_public self.caller r0 r1 into r2;
    // Output the finalize future.
    output r2 as credits.aleo/transfer_public.future;

finalize transfer_public:
    // Input the caller.
    input r0 as address.public;
    // Input the receiver.
    input r1 as address.public;
    // Input the amount.
    input r2 as u64.public;
    // Decrements `account[r0]` by `r2`.
    // If `account[r0] - r2` underflows, `transfer_public` is reverted.
    get account[r0] into r3;
    sub r3 r2 into r4;
    set r4 into account[r0];
    // Increments `account[r1]` by `r2`.
    // If `account[r1]` does not exist, 0u64 is used.
    // If `account[r1] + r2` overflows, `transfer_public` is reverted.
    get.or_use account[r1] 0u64 into r5;
    add r5 r2 into r6;
    set r6 into account[r1];

/**********************************************************************************************************************/

// The `transfer_public_as_signer` function sends the specified amount
// from the signer's `account` to the receiver's `account`.
function transfer_public_as_signer:
    // Input the receiver.
    input r0 as address.public;
    // Input the amount.
    input r1 as u64.public;
    // Transfer the credits publicly.
    async transfer_public_as_signer self.signer r0 r1 into r2;
    // Output the finalize future.
    output r2 as credits.aleo/transfer_public_as_signer.future;

finalize transfer_public_as_signer:
    // Input the signer.
    input r0 as address.public;
    // Input the receiver.
    input r1 as address.public;
    // Input the amount.
    input r2 as u64.public;
    // Decrements `account[r0]` by `r2`.
    // If `account[r0] - r2` underflows, `transfer_public_as_signer` is reverted.
    get account[r0] into r3;
    sub r3 r2 into r4;
    set r4 into account[r0];
    // Increments `account[r1]` by `r2`.
    // If `account[r1]` does not exist, 0u64 is used.
    // If `account[r1] + r2` overflows, `transfer_public_as_signer` is reverted.
    get.or_use account[r1] 0u64 into r5;
    add r5 r2 into r6;
    set r6 into account[r1];

/**********************************************************************************************************************/

// The `transfer_private` function sends the specified amount
// from the sender's record to the receiver in a record.
function transfer_private:
    // Input the sender's record.
    input r0 as credits.record;
    // Input the receiver.
    input r1 as address.private;
    // Input the amount.
    input r2 as u64.private;
    // Checks the given record has a sufficient amount.
    // This `sub` operation is safe, and the proof will fail
    // if an underflow occurs. The destination register `r3` holds
    // the change amount to be returned to the sender.
    sub r0.microcredits r2 into r3;
    // Construct a record for the specified receiver.
    cast r1 r2 into r4 as credits.record;
    // Construct a record with the change amount for the sender.
    cast r0.owner r3 into r5 as credits.record;
    // Output the receiver's record.
    output r4 as credits.record;
    // Output the sender's change record.
    output r5 as credits.record;

/**********************************************************************************************************************/

// The `transfer_private_to_public` function turns a specified amount
// from a record into public credits for the specified receiver.
//
// This function preserves privacy for the sender's record, however
// it publicly reveals the receiver and the amount.
function transfer_private_to_public:
    // Input the sender's record.
    input r0 as credits.record;
    // Input the receiver.
    input r1 as address.public;
    // Input the amount.
    input r2 as u64.public;
    // Checks the given record has a sufficient amount.
    // This `sub` operation is safe, and the proof will fail
    // if an underflow occurs. The destination register `r3` holds
    // the change amount for the sender.
    sub r0.microcredits r2 into r3;
    // Construct a record with the change amount for the sender.
    cast r0.owner r3 into r4 as credits.record;
    // Increment the amount publicly for the receiver.
    async transfer_private_to_public r1 r2 into r5;
    // Output the sender's change record.
    output r4 as credits.record;
    // Output the finalize future.
    output r5 as credits.aleo/transfer_private_to_public.future;

finalize transfer_private_to_public:
    // Input the receiver.
    input r0 as address.public;
    // Input the amount.
    input r1 as u64.public;
    // Retrieve the balance of the receiver.
    // If `account[r0]` does not exist, 0u64 is used.
    get.or_use account[r0] 0u64 into r2;
    // Increments `account[r0]` by `r1`.
    // If `r1 + r2` overflows, `transfer_private_to_public` is reverted.
    add r1 r2 into r3;
    // Updates the balance of the sender.
    set r3 into account[r0];

/**********************************************************************************************************************/

// The `transfer_public_to_private` function turns a specified amount
// from the mapping `account` into a record for the specified receiver.
//
// This function publicly reveals the sender, the receiver, and the specified amount.
// However, subsequent methods using the receiver's record can preserve the receiver's privacy.
function transfer_public_to_private:
    // Input the receiver.
    input r0 as address.private;
    // Input the amount.
    input r1 as u64.public;
    // Construct a record for the receiver.
    cast r0 r1 into r2 as credits.record;
    // Decrement the balance of the sender publicly.
    async transfer_public_to_private self.caller r1 into r3;
    // Output the record of the receiver.
    output r2 as credits.record;
    // Output the finalize future.
    output r3 as credits.aleo/transfer_public_to_private.future;

finalize transfer_public_to_private:
    // Input the sender.
    input r0 as address.public;
    // Input the amount.
    input r1 as u64.public;
    // Retrieve the balance of the sender.
    get account[r0] into r2;
    // Decrements `account[r0]` by `r1`.
    // If `r2 - r1` underflows, `transfer_public_to_private` is reverted.
    sub r2 r1 into r3;
    // Updates the balance of the sender.
    set r3 into account[r0];

/**********************************************************************************************************************/

// The `join` function combines two records into one.
function join:
    // Input the first record.
    input r0 as credits.record;
    // Input the second record.
    input r1 as credits.record;
    // Combines the amount of the first record and the second record.
    // This `add` operation is safe, and the proof will fail
    // if an overflow occurs.
    add r0.microcredits r1.microcredits into r2;
    // Construct a record with the combined amount.
    cast r0.owner r2 into r3 as credits.record;
    // Output the record.
    output r3 as credits.record;

/**********************************************************************************************************************/

// The `split` function splits a record into two records. The given input amount will be stored in the first record,
// and the remaining amount will be stored in the second record, with the fee deducted from the remaining amount.
// If the caller executes a transaction that contains only a call to this function, then the transaction does not
// require a fee, unless the caller wishes to provide an additional fee. Transactions that contain multiple transitions
// (that include one or more calls to this function) will require a fee as per standard consensus rules.
function split:
    // Input the record.
    input r0 as credits.record;
    // Input the amount to split.
    input r1 as u64.private;
    // Checks the given record has a sufficient amount to split.
    // This `sub` operation is safe, and the proof will fail
    // if an underflow occurs.
    sub r0.microcredits r1 into r2;
    // Checks the given record has a sufficient fee to remove.
    // This `sub` operation is safe, and the proof will fail
    // if an underflow occurs.
    sub r2 10_000u64 into r3;
    // Construct the first record.
    cast r0.owner r1 into r4 as credits.record;
    // Construct the second record.
    cast r0.owner r3 into r5 as credits.record;
    // Output the first record.
    output r4 as credits.record;
    // Output the second record.
    output r5 as credits.record;

/**********************************************************************************************************************/

// The `fee_private` function charges the specified amount from the sender's record.
function fee_private:
    // Input the sender's record.
    input r0 as credits.record;
    // Input the amount.
    input r1 as u64.public;
    // Input the priority fee amount.
    input r2 as u64.public;
    // Input the deployment or execution ID.
    input r3 as field.public;
    // Ensure the amount is nonzero.
    assert.neq r1 0u64;
    // Ensure the deployment or execution ID is nonzero.
    assert.neq r3 0field;
    // Add the fee and priority fee amounts.
    add r1 r2 into r4;
    // Checks the given record has a sufficient amount.
    // This `sub` operation is safe, and the proof will fail
    // if an underflow occurs. The destination register `r3` holds
    // the change amount for the sender.
    sub r0.microcredits r4 into r5;
    // Construct a record with the change amount for the sender.
    cast r0.owner r5 into r6 as credits.record;
    // Output the sender's change record.
    output r6 as credits.record;

/**********************************************************************************************************************/

// The `fee_public` function charges the specified amount from the sender's account.
function fee_public:
    // Input the amount.
    input r0 as u64.public;
    // Input the priority fee amount.
    input r1 as u64.public;
    // Input the deployment or execution ID.
    input r2 as field.public;
    // Ensure the amount is nonzero.
    assert.neq r0 0u64;
    // Ensure the deployment or execution ID is nonzero.
    assert.neq r2 0field;
    // Add the fee and priority fee amounts.
    add r0 r1 into r3;
    // Decrement the balance of the sender publicly.
    async fee_public self.signer r3 into r4;
    // Output the finalize future.
    output r4 as credits.aleo/fee_public.future;

finalize fee_public:
    // Input the sender's address.
    input r0 as address.public;
    // Input the total fee amount.
    input r1 as u64.public;
    // Retrieve the balance of the sender.
    // If `account[r0]` does not exist, `fee_public` is reverted.
    get account[r0] into r2;
    // Decrements `account[r0]` by `r1`.
    // If `r2 - r1` underflows, `fee_public` is reverted.
    sub r2 r1 into r3;
    // Updates the balance of the sender.
    set r3 into account[r0];

/**********************************************************************************************************************/
