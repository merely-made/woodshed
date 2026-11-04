//! Application custody needs; the resident owner grants them separately.

use ostiary::declaration::{
    AppDeclaration, CustodyOperation, CustodyUse, NamePattern, SaltPattern, SaltUse,
};

pub fn app_declaration() -> AppDeclaration {
    AppDeclaration {
        routes: vec![NamePattern::exact("custody")],
        custody: CustodyUse {
            operations: [
                CustodyOperation::Status,
                CustodyOperation::Roster,
                CustodyOperation::WatchLock,
                CustodyOperation::Release,
                CustodyOperation::Attest,
            ]
            .into(),
            salts: vec![SaltUse::persona(SaltPattern::Prefix(
                "hocket/handoff/v2/".into(),
            ))],
            ..CustodyUse::default()
        },
        ..AppDeclaration::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ostiary::app_admission::{AppId, AppRouteId};
    use ostiary::declaration::{
        AppAuthority, AppGrant, AuthorityRefusal, CustodyOperation, SaltSource,
    };

    fn granted() -> AppAuthority {
        let declaration = app_declaration();
        declaration.validate().unwrap();
        let mut grant = AppGrant::new(AppId::new("hocket"));
        grant.routes = declaration.routes.clone();
        grant.custody.operations = declaration.custody.operations.clone();
        grant.custody.salts = declaration.custody.salts.clone();
        AppAuthority::new(declaration, grant)
    }

    #[test]
    fn custody_is_declared_but_projection_and_vault_administration_are_not() {
        let authority = granted();
        assert!(
            authority
                .declaration()
                .declares_route(&AppRouteId::new("custody").unwrap())
        );
        assert!(
            !authority
                .declaration()
                .declares_route(&AppRouteId::new("mere").unwrap())
        );
        for operation in [
            CustodyOperation::Status,
            CustodyOperation::Roster,
            CustodyOperation::WatchLock,
            CustodyOperation::Release,
            CustodyOperation::Attest,
        ] {
            authority.custody_operation(operation).unwrap();
        }
        for operation in [
            CustodyOperation::UnlockNative,
            CustodyOperation::CreateProfile,
            CustodyOperation::IssueStationGrant,
            CustodyOperation::Payload,
        ] {
            assert!(matches!(
                authority.custody_operation(operation),
                Err(AuthorityRefusal::Undeclared { .. })
            ));
        }
    }

    #[test]
    fn released_keys_stay_in_this_apps_persona_namespace() {
        let authority = granted();
        // SessionId is a binary UUID, not a UTF-8 suffix.
        let mut salt = b"hocket/handoff/v2/".to_vec();
        salt.extend_from_slice(&[0xff; 16]);
        authority.salt(SaltSource::Persona, &salt, true).unwrap();
        authority.salt(SaltSource::Persona, &salt, false).unwrap();
        assert!(matches!(
            authority.salt(SaltSource::Persona, b"hocket/handoff/v2/", true),
            Err(AuthorityRefusal::Undeclared { .. })
        ));
        assert!(matches!(
            authority.custody_operation(CustodyOperation::ChooseProfile),
            Err(AuthorityRefusal::Undeclared { .. })
        ));
        assert!(matches!(
            authority.salt(
                SaltSource::Persona,
                b"woodshed.practice-session.seal.v1",
                true
            ),
            Err(AuthorityRefusal::Undeclared { .. })
        ));
        assert!(matches!(
            authority.salt(
                SaltSource::Wallet,
                b"hocket/handoff/v2/0123456789abcdef",
                true
            ),
            Err(AuthorityRefusal::Undeclared { .. })
        ));
    }

    #[test]
    fn a_declaration_cannot_grant_itself_custody() {
        let authority = AppAuthority::new(app_declaration(), AppGrant::new(AppId::new("hocket")));
        assert!(matches!(
            authority.custody_operation(CustodyOperation::Release),
            Err(AuthorityRefusal::NotGranted { .. })
        ));
        assert!(matches!(
            authority.salt(
                SaltSource::Persona,
                b"hocket/handoff/v2/0123456789abcdef",
                true
            ),
            Err(AuthorityRefusal::NotGranted { .. })
        ));
    }
}
