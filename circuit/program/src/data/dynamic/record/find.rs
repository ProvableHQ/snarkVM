// Copyright (c) 2019-2026 Provable Inc.
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

use super::*;

impl<A: Aleo> DynamicRecord<A> {
    /// Returns the entry from the given path.
    pub fn find<A0: Into<Access<A>> + Clone + Debug>(&self, path: &[A0]) -> Result<Value<A>> {
        // Check the nonce before the `owner`: comparing `Access` values ejects them, and ejecting `_nonce` halts.
        // Building a constant identifier for this check would add counted variables on every one-segment record access.
        if let [access] = path
            && let Access::Member(identifier) = access.clone().into()
            && identifier.to_field().eject_value()
                == console::ToField::to_field(&console::Identifier::<A::Network>::record_nonce()?)?
        {
            Ok(Value::Plaintext(Plaintext::from(Literal::Group(self.nonce.clone()))))
        }
        // If the path is of length one, check if the path is requesting the `owner`.
        else if path.len() == 1 && path[0].clone().into() == Access::Member(Identifier::from_str("owner")?) {
            Ok(Value::Plaintext(Plaintext::from(Literal::Address(self.owner.clone()))))
        } else {
            bail!(
                "Only the 'owner' or '_nonce' of a dynamic record can be accessed directly, use 'get.record.dynamic' for other entries."
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Circuit;

    type CurrentNetwork = <Circuit as Environment>::Network;

    /// Counts added by one circuit scope: constants, public, private, constraints.
    fn scope_counts() -> (u64, u64, u64, u64) {
        (
            Circuit::num_constants_in_scope(),
            Circuit::num_public_in_scope(),
            Circuit::num_private_in_scope(),
            Circuit::num_constraints_in_scope(),
        )
    }

    #[test]
    fn test_find_member_does_not_allocate_nonce_variables() -> Result<()> {
        let console_record = console::Record::<CurrentNetwork, console::Plaintext<CurrentNetwork>>::from_str(
            r"{
    owner: aleo14tlamssdmg3d0p5zmljma573jghe2q9n6wz29qf36re2glcedcpqfg4add.private,
    amount: 5u64.private,
    _nonce: 2293253577170800572742339369209137467208538700597121244293392265726446806023group.public
}",
        )?;
        let console_dynamic = console::DynamicRecord::from_record(&console_record)?;
        let expected_nonce = *console_dynamic.nonce();
        let record = DynamicRecord::<Circuit>::new(Mode::Private, console_dynamic);
        let nonce_access = Access::constant(console::Access::Member(console::Identifier::record_nonce()?));
        let amount_access = Access::constant(console::Access::Member(console::Identifier::from_str("amount")?));
        let owner_access = Access::constant(console::Access::Member(console::Identifier::from_str("owner")?));

        // A one-segment member read allocates only the `owner` identifier used by the owner check.
        let owner_counts = Circuit::scope("owner identifier", || -> Result<_> {
            let _owner = Identifier::<Circuit>::from_str("owner")?;
            Ok(scope_counts())
        })?;

        // A nonce read returns before that comparison, so it adds no variables.
        let nonce_counts = Circuit::scope("find nonce", || -> Result<_> {
            match record.find(&[nonce_access])? {
                Value::Plaintext(Plaintext::Literal(Literal::Group(nonce), _)) => {
                    assert_eq!(expected_nonce, nonce.eject_value());
                }
                _ => bail!("Expected the record nonce to be a private group entry"),
            }
            Ok(scope_counts())
        })?;
        assert_eq!(nonce_counts, (0, 0, 0, 0));

        // `owner` adds the same variables as the `owner` identifier alone.
        // Another member still builds that identifier, then rejects the access.
        let owner_find_counts = Circuit::scope("find owner", || -> Result<_> {
            let _owner = record.find(&[owner_access])?;
            Ok(scope_counts())
        })?;
        assert_eq!(owner_find_counts, owner_counts);

        let amount_counts = Circuit::scope("find amount", || -> Result<_> {
            assert!(record.find(&[amount_access]).is_err());
            Ok(scope_counts())
        })?;
        assert_eq!(amount_counts, owner_counts);

        Circuit::reset();
        Ok(())
    }
}
