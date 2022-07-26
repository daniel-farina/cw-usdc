use cosmwasm_std::{coins, BankMsg};
#[cfg(not(feature = "library"))]
use cosmwasm_std::{DepsMut, Env, MessageInfo, Response, StdResult, Uint128};
use osmo_bindings::OsmosisMsg;

use crate::error::ContractError;
use crate::helpers::check_is_contract_owner;
use crate::state::{
    Config, BLACKLISTED_ADDRESSES, BLACKLISTER_ALLOWANCES, BURNER_ALLOWANCES, CONFIG,
    FREEZER_ALLOWANCES, MINTER_ALLOWANCES,
};

pub fn mint(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    to_address: String,
    amount: Uint128,
) -> Result<Response<OsmosisMsg>, ContractError> {
    // validate that to_address is a valid address
    deps.api.addr_validate(&to_address)?;

    // don't allow minting of 0 coins
    if amount.eq(&Uint128::new(0_u128)) {
        return Result::Err(ContractError::ZeroAmount {});
    }

    // decrease minter allowance
    // if minter allowance goes negative, throw error
    let _allowance = MINTER_ALLOWANCES.update(
        deps.storage,
        &info.sender,
        |allowance| -> StdResult<Uint128> {
            Ok(allowance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;

    // get token denom from contract config
    let denom = CONFIG.load(deps.storage).unwrap().denom;

    // create tokenfactory MsgMint which mints coins to the contract address
    let mint_tokens_msg =
        OsmosisMsg::mint_contract_tokens(denom.clone(), amount, env.contract.address.into_string());

    // send newly minted coins from contract to designated recipient
    let send_tokens_msg = BankMsg::Send {
        to_address,
        amount: coins(amount.u128(), denom),
    };

    // dispatch msgs
    Ok(Response::new()
        .add_attribute("method", "mint_tokens")
        .add_message(mint_tokens_msg)
        .add_message(send_tokens_msg))
}

pub fn burn(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response<OsmosisMsg>, ContractError> {
    // don't allow burning of 0 coins
    if amount.eq(&Uint128::new(0_u128)) {
        return Result::Err(ContractError::ZeroAmount {});
    }

    // decrease burner allowance
    // if burner allowance goes negative, throw error
    let _allowance = BURNER_ALLOWANCES.update(
        deps.storage,
        &info.sender,
        |allowance| -> StdResult<Uint128> {
            Ok(allowance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;

    // get token denom from contract config
    let denom = CONFIG.load(deps.storage).unwrap().denom;

    // create tokenfactory MsgBurn which burns coins from the contract address
    // NOTE: this requires the contract to own the tokens already
    let burn_tokens_msg = OsmosisMsg::burn_contract_tokens(denom, amount, "".to_string());

    // dispatch msg
    Ok(Response::new()
        .add_attribute("method", "execute_burn")
        .add_attribute("amount", amount.to_string())
        .add_message(burn_tokens_msg))
}

pub fn change_contract_owner(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_owner: String,
) -> Result<Response<OsmosisMsg>, ContractError> {
    // Only allow current contract owner to change owner
    check_is_contract_owner(deps.as_ref(), info.sender)?;

    // validate that new owner is a valid address
    let new_owner_addr = deps.api.addr_validate(new_owner.as_str())?;

    // update the contract owner in the contract config
    CONFIG.update(
        deps.storage,
        |mut config: Config| -> Result<Config, ContractError> {
            config.owner = new_owner_addr;
            Ok(config)
        },
    )?;

    // return OK
    Ok(Response::new()
        .add_attribute("method", "change_contract_owner")
        .add_attribute("new_owner", new_owner))
}

pub fn change_tokenfactory_admin(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_admin_address: String,
) -> Result<Response<OsmosisMsg>, ContractError> {
    // Only allow current contract owner to change tokenfactory admin
    check_is_contract_owner(deps.as_ref(), info.sender)?;

    // validate that the new admin is a valid address
    deps.api.addr_validate(new_admin_address.as_str())?;

    // construct tokenfactory change admin msg
    let change_admin_msg = OsmosisMsg::ChangeAdmin {
        denom: CONFIG.load(deps.storage).unwrap().denom,
        new_admin_address,
    };

    // dispatch change admin msg
    Ok(Response::new()
        .add_attribute("method", "change_tokenfactory_admin") // TODO: add more events
        .add_message(change_admin_msg))
}

pub fn set_blacklister(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: String,
    status: bool,
) -> Result<Response<OsmosisMsg>, ContractError> {
    // Only allow current contract owner to set blacklister permission
    check_is_contract_owner(deps.as_ref(), info.sender)?;

    // set blacklister status
    // NOTE: Does not check if new status is same as old status
    BLACKLISTER_ALLOWANCES.update(
        deps.storage,
        &deps.api.addr_validate(&address)?,
        |_| -> StdResult<bool> { Ok(status) },
    )?;

    // Return OK
    Ok(Response::new()
        .add_attribute("method", "set_blacklister")
        .add_attribute("blacklister", address)
        .add_attribute("status", status.to_string()))
}

pub fn set_freezer(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: String,
    status: bool,
) -> Result<Response<OsmosisMsg>, ContractError> {
    // Only allow current contract owner to set freezer permission
    check_is_contract_owner(deps.as_ref(), info.sender)?;

    // set freezer status
    // NOTE: Does not check if new status is same as old status
    FREEZER_ALLOWANCES.update(
        deps.storage,
        &deps.api.addr_validate(&address)?,
        |_| -> StdResult<bool> { Ok(status) },
    )?;

    // return OK
    Ok(Response::new()
        .add_attribute("method", "set_freezer")
        .add_attribute("freezer", address)
        .add_attribute("status", status.to_string()))
}

pub fn set_burner(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: String,
    allowance: Uint128,
) -> Result<Response<OsmosisMsg>, ContractError> {
    // Only allow current contract owner to set burner allowance
    check_is_contract_owner(deps.as_ref(), info.sender)?;

    // update allowance of burner
    // validate that burner is a valid address
    BURNER_ALLOWANCES.save(deps.storage, &deps.api.addr_validate(&address)?, &allowance)?;

    // return OK
    Ok(Response::new()
        .add_attribute("method", "set_burner")
        .add_attribute("burner", address)
        .add_attribute("allowance", allowance))
}

pub fn set_minter(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: String,
    allowance: Uint128,
) -> Result<Response<OsmosisMsg>, ContractError> {
    // Only allow current contract owner to set minter allowance
    check_is_contract_owner(deps.as_ref(), info.sender)?;

    // update allowance of minter
    // validate that minter is a valid address
    MINTER_ALLOWANCES.save(deps.storage, &deps.api.addr_validate(&address)?, &allowance)?;

    // return OK
    Ok(Response::new()
        .add_attribute("method", "set_minter")
        .add_attribute("minter", address)
        .add_attribute("amount", allowance))
}

pub fn freeze(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    status: bool,
) -> Result<Response<OsmosisMsg>, ContractError> {
    // check to make sure that the sender has freezer permissions
    let res = FREEZER_ALLOWANCES.may_load(deps.storage, &info.sender)?;
    match res {
        Some(true) => (),
        _ => return Err(ContractError::Unauthorized {}),
    }

    // Update config frozen status
    // NOTE: Does not check if new status is same as old status
    CONFIG.update(
        deps.storage,
        |mut config: Config| -> Result<_, ContractError> {
            config.is_frozen = status;
            Ok(config)
        },
    )?;

    // return OK
    Ok(Response::new()
        .add_attribute("method", "execute_freeze")
        .add_attribute("status", status.to_string()))
}

pub fn blacklist(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: String,
    status: bool,
) -> Result<Response<OsmosisMsg>, ContractError> {
    // check to make sure that the sender has blacklister permissions
    let res = BLACKLISTER_ALLOWANCES.may_load(deps.storage, &info.sender)?;
    match res {
        Some(true) => (),
        _ => return Err(ContractError::Unauthorized {}),
    }

    // update blacklisted status
    // validate that blacklisteed is a valid address
    // NOTE: Does not check if new status is same as old status
    BLACKLISTED_ADDRESSES.update(
        deps.storage,
        &deps.api.addr_validate(address.as_str())?,
        |mut _stat| -> Result<_, ContractError> {
            _stat = Some(status);
            Ok(status)
        },
    )?;

    // return OK
    Ok(Response::new()
        .add_attribute("method", "blacklist")
        .add_attribute("address", address)
        .add_attribute("status", status.to_string()))
}
